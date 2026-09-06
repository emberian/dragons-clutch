//! Account lifecycle: rent quotes, static register ownership, the lifecycle
//! preplan and replan, funded creates and closes.

use super::*;

/// Materialize current-Rent facts only from the authenticated Rent sysvar and
/// the exact V5 declarations selected by the capability descriptor.
#[inline(never)]
pub(super) fn authenticate_current_rent_quotes_v5(
    policy: StateLifecyclePolicyV5<'_>,
    rent: &Rent,
    action: u32,
) -> Result<Vec<AuthenticatedRentQuoteV5>, ProgramError> {
    // One quote per declaration THIS action projects, in declaration order --
    // the subsequence both lifecycle walkers expect. A declaration scoped to a
    // sibling action is skipped here and never reaches the bank, which is what
    // lets one policy serve a family whose actions open different children.
    let mut quotes = Vec::with_capacity(usize::from(policy.current_rent_quote_count()));
    let mut ordinal = 0_u16;
    while ordinal < policy.current_rent_quote_count() {
        let declaration = policy
            .current_rent_quote(ordinal)
            .map_err(|_| TradingSbfError::Content)?;
        if !declaration.applies_to(action) {
            ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
            continue;
        }
        let exact_data_len = declaration.exact_data_len();
        quotes.push(AuthenticatedRentQuoteV5 {
            exact_data_len,
            scalar_destination: declaration.scalar_destination().index(),
            current_minimum: rent.minimum_balance(
                usize::try_from(exact_data_len).map_err(|_| TradingSbfError::Content)?,
            ),
        });
        ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(quotes)
}

/// Every artifact whose static register ownership is checked together.
pub(super) struct StaticRegisterOwnershipV5<'a> {
    pub(super) account_profile: AccountProfileV2<'a>,
    pub(super) policy: StateLifecyclePolicyV5<'a>,
    pub(super) action: u32,
    pub(super) request: RequestProfileKindV3<'a>,
    pub(super) transition: TransitionProgramV3<'a>,
}

/// Require that no register the lifecycle policy, the trusted-environment
/// observation, or a dynamic fixed span owns is also written by the request
/// profile or by the transition program.
///
/// The three predicates this replaces each asked one target at a time, and
/// both `writes_register` implementations answer a single target by decoding
/// every operation of their whole program. Over the Direct Profile14 lifecycle
/// that is a few dozen full passes of a 66-instruction transition and of the
/// request profile. Every target is collected first - the structural
/// requirements on each plan are still checked while collecting - and each
/// artifact is then walked exactly once for the entire set. The accepted set
/// is unchanged: a target is refused here if and only if
/// `writes_register` would have reported it before.
#[inline(never)]
pub(super) fn require_static_register_ownership_v5(
    input: StaticRegisterOwnershipV5<'_>,
) -> Result<(), ProgramError> {
    let StaticRegisterOwnershipV5 {
        account_profile,
        policy,
        action,
        request,
        transition,
    } = input;
    let plan_count = policy
        .action_plan_count(action)
        .map_err(|_| TradingSbfError::Content)?;
    // Exact upper bounds, so neither bank walks the bump allocator's doubling
    // ladder: rent quotes and three trusted-environment registers are forbidden
    // to both artifacts, and every per-plan lifecycle register plus every
    // dynamic-span count scalar is additionally forbidden to the transition.
    let shared_bound = usize::from(policy.current_rent_quote_count())
        .checked_add(3)
        .ok_or(TradingSbfError::Content)?;
    let mut transition_bound = shared_bound;
    if account_profile.uses_dynamic_fixed_spans() {
        transition_bound = transition_bound
            .checked_add(usize::from(account_profile.dynamic_fixed_span_count()))
            .ok_or(TradingSbfError::Content)?;
    }
    let mut request_bound = shared_bound;
    let mut counted = 0_u16;
    while counted < plan_count {
        let selected = policy
            .action_plan(action, counted)
            .map_err(|_| TradingSbfError::Content)?;
        for width in [
            usize::from(
                selected
                    .protected_observation_count()
                    .map_err(|_| TradingSbfError::Content)?,
            ),
            usize::from(
                selected
                    .protected_output_count()
                    .map_err(|_| TradingSbfError::Content)?,
            ),
        ] {
            request_bound = request_bound
                .checked_add(width)
                .ok_or(TradingSbfError::Content)?;
            transition_bound = transition_bound
                .checked_add(width)
                .ok_or(TradingSbfError::Content)?;
        }
        for width in [
            usize::from(
                selected
                    .seed_count()
                    .map_err(|_| TradingSbfError::Content)?,
            ),
            usize::from(
                selected
                    .immutable_identity_binding_count()
                    .map_err(|_| TradingSbfError::Content)?,
            ),
        ] {
            transition_bound = transition_bound
                .checked_add(width)
                .ok_or(TradingSbfError::Content)?;
        }
        counted = counted.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    let mut request_forbidden = Vec::with_capacity(request_bound);
    let mut transition_forbidden: Vec<RegisterWriteTargetV3> = Vec::with_capacity(transition_bound);

    let mut quote = 0_u16;
    while quote < policy.current_rent_quote_count() {
        let target = policy
            .current_rent_quote(quote)
            .map_err(|_| TradingSbfError::Content)?
            .scalar_destination();
        request_forbidden.push(lifecycle_request_target_v4(target));
        transition_forbidden.push(lifecycle_transition_target_v4(target));
        quote = quote.checked_add(1).ok_or(TradingSbfError::Content)?;
    }

    for (index, register) in [
        account_profile.trusted_current_slot_scalar(),
        account_profile.trusted_current_executing_program_identity(),
        account_profile.trusted_system_program_identity(),
    ]
    .into_iter()
    .zip([
        ProjectionRegisterKindV1::Scalar,
        ProjectionRegisterKindV1::Identity,
        ProjectionRegisterKindV1::Identity,
    ]) {
        let Some(index) = index else {
            continue;
        };
        request_forbidden.push(ProjectionTargetV1 {
            kind: register,
            space: ProjectionRegisterSpaceV1::Common,
            index,
        });
        transition_forbidden.push(RegisterWriteTargetV3 {
            kind: match register {
                ProjectionRegisterKindV1::Scalar => RegisterKindV3::Scalar,
                ProjectionRegisterKindV1::Identity => RegisterKindV3::Identity,
            },
            space: RegisterSpaceV3::Common,
            index,
        });
    }

    if account_profile.uses_dynamic_fixed_spans() {
        let mut span = 0_u16;
        while span < account_profile.dynamic_fixed_span_count() {
            transition_forbidden.push(RegisterWriteTargetV3 {
                kind: RegisterKindV3::Scalar,
                space: RegisterSpaceV3::Common,
                index: account_profile
                    .dynamic_fixed_span(span)
                    .map_err(|_| TradingSbfError::Content)?
                    .count_scalar(),
            });
            span = span.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
    }

    let mut ordinal = 0_u16;
    while ordinal < plan_count {
        let selected = policy
            .action_plan(action, ordinal)
            .map_err(|_| TradingSbfError::Content)?;
        if selected.operation() != LifecycleOperationV3::AuthenticateOrCreate
            || selected
                .protected_output_count()
                .map_err(|_| TradingSbfError::Content)?
                != 6
        {
            return Err(TradingSbfError::Content.into());
        }
        let mut observation = 0_u8;
        while observation
            < selected
                .protected_observation_count()
                .map_err(|_| TradingSbfError::Content)?
        {
            let target = selected
                .protected_observation_target(observation)
                .map_err(|_| TradingSbfError::Content)?;
            request_forbidden.push(lifecycle_request_target_v4(target));
            transition_forbidden.push(lifecycle_transition_target_v4(target));
            observation = observation.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        let mut output = 0_u8;
        while output
            < selected
                .protected_output_count()
                .map_err(|_| TradingSbfError::Content)?
        {
            let target = selected
                .protected_output_target(output)
                .map_err(|_| TradingSbfError::Content)?;
            request_forbidden.push(lifecycle_request_target_v4(target));
            transition_forbidden.push(lifecycle_transition_target_v4(target));
            output = output.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        let mut seed = 0_u8;
        while seed
            < selected
                .seed_count()
                .map_err(|_| TradingSbfError::Content)?
        {
            if let Some(target) = selected
                .seed_register_target(seed)
                .map_err(|_| TradingSbfError::Content)?
            {
                transition_forbidden.push(lifecycle_transition_target_v4(target));
            }
            seed = seed.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        let mut binding = 0_u16;
        while binding
            < selected
                .immutable_identity_binding_count()
                .map_err(|_| TradingSbfError::Content)?
        {
            transition_forbidden.push(lifecycle_transition_target_v4(
                selected
                    .immutable_identity_binding(binding)
                    .map_err(|_| TradingSbfError::Content)?
                    .canonical(),
            ));
            binding = binding.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
    }

    if request.writes_any_register(&request_forbidden)?
        || transition
            .writes_any_register(&transition_forbidden)
            .map_err(|_| TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct PreparedImmutableIdentityBindingV4 {
    pub(super) data_offset: u32,
    pub(super) canonical: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct PreparedLifecycleInvocationV3 {
    pub(super) plan: StateLifecyclePlanV3,
    pub(super) state: usize,
    pub(super) payer: Option<usize>,
    pub(super) rent_credit: Option<usize>,
    pub(super) seeds: Vec<Vec<u8>>,
    pub(super) immutable_identity_bindings: Vec<PreparedImmutableIdentityBindingV4>,
}

pub(super) struct PreparedLifecycleBatchV4<'region> {
    pub(super) plans: Vec<PreparedLifecycleInvocationV3>,
    pub(super) scalars: ScratchVecV1<'region, u64>,
    pub(super) identities: ScratchVecV1<'region, [u8; 32]>,
}

/// The root is an ordinary live CapabilityRoot unless the selected lifecycle
/// table itself owns its terminal close. Merely mentioning coordinate zero is
/// never enough: Authenticate and Create retain the historical refusal.
pub(super) fn selected_root_lifecycle_close_v3(
    plans: &[PreparedLifecycleInvocationV3],
) -> Result<bool, ProgramError> {
    let mut selected = false;
    for prepared in plans {
        if prepared.state != 0 {
            continue;
        }
        if selected || !matches!(prepared.plan, StateLifecyclePlanV3::Close(_)) {
            return Err(TradingSbfError::Transition.into());
        }
        selected = true;
    }
    Ok(selected)
}

/// Where one prepared lifecycle invocation goes.
///
/// # Why the preparation runs twice, and what the second run actually has to do
///
/// `prepare_lifecycle_v4` is a pure function of its artifacts, the accounts,
/// and one pair of register banks. The preplan evaluates it at the **request**
/// registers and hands the resulting plan table to the transition. The replan
/// evaluates it at the **transition's own output** registers, and the pair of
/// them assert a fixed point: the transition's outputs must reproduce the plan
/// table the transition was given, and must be unchanged by the lifecycle
/// projection applied to them. That is not redundancy - a transition that
/// rewrote a coordinate the plan reads would otherwise execute against a plan
/// nobody ever validated - and there is no way to answer it except by
/// evaluating the function at the transition's outputs.
///
/// What the second evaluation does **not** have to do is build a second copy of
/// an answer it is only going to compare. So it does not: the replan verifies
/// against the preplan's table as it goes and allocates nothing per invocation,
/// where before it allocated a fresh plan vector, a `Vec<Vec<u8>>` of seeds and
/// a `Vec<&[u8]>` of slices per invocation, and a binding vector - on an
/// allocator whose `dealloc` is a no-op, all of it charged against
/// total-ever-allocated for the lifetime of the instruction.
///
/// The one derivation it also skips is named at [`LifecycleSeedsV4::pending_bump`].
pub(super) enum LifecycleBatchSinkV4<'a> {
    /// The preplan: collect the table the transition will be handed.
    Collect(Vec<PreparedLifecycleInvocationV3>),
    /// The replan: reproduce the table the preplan already produced.
    Verify {
        expected: &'a [PreparedLifecycleInvocationV3],
        next: usize,
    },
}

impl<'a> LifecycleBatchSinkV4<'a> {
    /// Reserve the exact table width the plan declares.
    pub(super) fn new(
        expected: Option<&'a [PreparedLifecycleInvocationV3]>,
        planned: usize,
    ) -> Result<Self, ProgramError> {
        match expected {
            None => {
                // Exact capacity: the plan table declares how many invocations
                // the batch has, so the output bank does not walk the
                // allocator's doubling ladder.
                let mut output = Vec::new();
                output
                    .try_reserve_exact(planned)
                    .map_err(|_| TradingSbfError::HeapExhausted)?;
                Ok(Self::Collect(output))
            }
            Some(expected) => {
                if expected.len() != planned {
                    return Err(TradingSbfError::Transition.into());
                }
                Ok(Self::Verify { expected, next: 0 })
            }
        }
    }

    /// The already-prepared invocation this ordinal must reproduce, if verifying.
    fn expected(&self) -> Result<Option<&'a PreparedLifecycleInvocationV3>, ProgramError> {
        match self {
            Self::Collect(_) => Ok(None),
            Self::Verify { expected, next } => Ok(Some(
                expected.get(*next).ok_or(TradingSbfError::Transition)?,
            )),
        }
    }

    /// Admit one complete invocation, or refuse it against the preplan's.
    pub(super) fn admit(
        &mut self,
        plan: StateLifecyclePlanV3,
        state: usize,
        payer: Option<usize>,
        rent_credit: Option<usize>,
        seeds: LifecycleSeedsV4<'_>,
        bindings: LifecycleBindingsV4<'_>,
    ) -> Result<(), ProgramError> {
        match self {
            Self::Collect(output) => {
                output.push(PreparedLifecycleInvocationV3 {
                    plan,
                    state,
                    payer,
                    rent_credit,
                    seeds: seeds.collected()?,
                    immutable_identity_bindings: bindings.collected()?,
                });
                Ok(())
            }
            Self::Verify { expected, next } => {
                let prior = expected.get(*next).ok_or(TradingSbfError::Transition)?;
                // Seeds and bindings were compared element by element as they
                // were materialized; what is left is that every element was in
                // fact reached.
                seeds.exhausted()?;
                bindings.exhausted()?;
                if prior.plan != plan
                    || prior.state != state
                    || prior.payer != payer
                    || prior.rent_credit != rent_credit
                {
                    return Err(TradingSbfError::Transition.into());
                }
                *next = next.checked_add(1).ok_or(TradingSbfError::Transition)?;
                Ok(())
            }
        }
    }

    /// The collected table, or an empty one when this pass only verified.
    pub(super) fn finish(
        self,
        planned: usize,
    ) -> Result<Vec<PreparedLifecycleInvocationV3>, ProgramError> {
        match self {
            Self::Collect(output) => {
                if output.len() != planned {
                    return Err(TradingSbfError::Content.into());
                }
                Ok(output)
            }
            Self::Verify { expected, next } => {
                if next != expected.len() {
                    return Err(TradingSbfError::Transition.into());
                }
                // The table this pass agreed with is the caller's own; handing
                // back a duplicate of it is the allocation this pass exists to
                // not make.
                Ok(Vec::new())
            }
        }
    }
}

/// One invocation's canonical seed vector, collected or verified.
pub(super) enum LifecycleSeedsV4<'a> {
    Collect(Vec<Vec<u8>>),
    Verify {
        expected: &'a [Vec<u8>],
        next: usize,
    },
}

/// Where one invocation's canonical bump came from.
pub(super) enum LifecycleCanonicalBumpV4 {
    /// Derived here, against the seeds this pass materialized.
    Derived { address: Pubkey, bump: u8 },
    /// Taken from the preplan's derivation over byte-identical seeds.
    Reused { bump: u8 },
}

impl<'a> LifecycleSeedsV4<'a> {
    pub(super) fn new(
        expected: Option<&'a [Vec<u8>]>,
        seed_count: u8,
    ) -> Result<Self, ProgramError> {
        match expected {
            None => Ok(Self::Collect(Vec::with_capacity(usize::from(seed_count)))),
            Some(expected) => {
                if expected.len() != usize::from(seed_count) {
                    return Err(TradingSbfError::Transition.into());
                }
                Ok(Self::Verify { expected, next: 0 })
            }
        }
    }

    /// Admit one materialized seed, or refuse it against the preplan's.
    pub(super) fn push(&mut self, bytes: &[u8]) -> Result<(), ProgramError> {
        match self {
            Self::Collect(seeds) => {
                seeds.push(bytes.to_vec());
                Ok(())
            }
            Self::Verify { expected, next } => {
                if expected.get(*next).map(Vec::as_slice) != Some(bytes) {
                    return Err(TradingSbfError::Transition.into());
                }
                *next = next.checked_add(1).ok_or(TradingSbfError::Transition)?;
                Ok(())
            }
        }
    }

    /// The canonical bump for the seeds pushed so far.
    ///
    /// The preplan REPRODUCES it from the caller's mined hint where there is
    /// one, and searches only where there is none. A lifecycle-created state is
    /// the hardest address on this route to carry a stored bump for -- it does
    /// not exist yet, so no account can have recorded it -- and it is also the
    /// easiest to mine off chain, because its seeds are materialized from
    /// registers the caller already computed to build the request at all. A
    /// wrong hint reproduces a different address, which is compared against the
    /// state coordinate the frame supplied and refused; see the equality on
    /// `derived` in `prepare_lifecycle_v4`.
    ///
    /// **The replan does not derive at all**, and that remains the one
    /// recomputation the second pass skips outright:
    /// [`Pubkey::try_find_program_address`] is a pure function of the seed
    /// bytes and the program id, every one of those bytes has just been
    /// compared byte-for-byte against the seeds the preplan derived from, and a
    /// divergence in any of them refuses at [`Self::push`] before this is ever
    /// reached. Re-running the SHA-256 ladder can only reproduce a value the
    /// caller already holds, at a syscall per attempt.
    ///
    /// The address is not reconstructed either: the preplan checked its own
    /// derivation against the state account's key, so the caller reads it off
    /// that account, and the caller has already required the state coordinate
    /// to be the preplan's.
    pub(super) fn pending_bump(
        &self,
        program_id: &Pubkey,
        hint: u8,
    ) -> Result<LifecycleCanonicalBumpV4, ProgramError> {
        match self {
            Self::Collect(seeds) => {
                let mut seed_slices = seeds.iter().map(Vec::as_slice).collect::<Vec<_>>();
                let Some(bump) = hot_bump_hint_v1(hint) else {
                    let (address, bump) =
                        Pubkey::try_find_program_address(seed_slices.as_slice(), program_id)
                            .ok_or(TradingSbfError::Content)?;
                    return Ok(LifecycleCanonicalBumpV4::Derived { address, bump });
                };
                let bump_seed = [bump];
                seed_slices.push(bump_seed.as_slice());
                let address = Pubkey::create_program_address(seed_slices.as_slice(), program_id)
                    .map_err(|_| TradingSbfError::Content)?;
                Ok(LifecycleCanonicalBumpV4::Derived { address, bump })
            }
            Self::Verify { expected, next } => {
                let [bump] = expected
                    .get(*next)
                    .ok_or(TradingSbfError::Transition)?
                    .as_slice()
                else {
                    return Err(TradingSbfError::Transition.into());
                };
                Ok(LifecycleCanonicalBumpV4::Reused { bump: *bump })
            }
        }
    }

    pub(super) fn collected(self) -> Result<Vec<Vec<u8>>, ProgramError> {
        match self {
            Self::Collect(seeds) => Ok(seeds),
            Self::Verify { .. } => Err(TradingSbfError::Transition.into()),
        }
    }

    pub(super) fn exhausted(&self) -> Result<(), ProgramError> {
        match self {
            Self::Collect(_) => Err(TradingSbfError::Transition.into()),
            Self::Verify { expected, next } => {
                if *next == expected.len() {
                    Ok(())
                } else {
                    Err(TradingSbfError::Transition.into())
                }
            }
        }
    }
}

/// One invocation's immutable identity bindings, collected or verified.
pub(super) enum LifecycleBindingsV4<'a> {
    Collect(Vec<PreparedImmutableIdentityBindingV4>),
    Verify {
        expected: &'a [PreparedImmutableIdentityBindingV4],
        next: usize,
    },
}

impl<'a> LifecycleBindingsV4<'a> {
    pub(super) fn new(
        expected: Option<&'a [PreparedImmutableIdentityBindingV4]>,
        count: u16,
    ) -> Result<Self, ProgramError> {
        match expected {
            None => Ok(Self::Collect(Vec::with_capacity(usize::from(count)))),
            Some(expected) => {
                if expected.len() != usize::from(count) {
                    return Err(TradingSbfError::Transition.into());
                }
                Ok(Self::Verify { expected, next: 0 })
            }
        }
    }

    pub(super) fn push(
        &mut self,
        binding: PreparedImmutableIdentityBindingV4,
    ) -> Result<(), ProgramError> {
        match self {
            Self::Collect(output) => {
                output.push(binding);
                Ok(())
            }
            Self::Verify { expected, next } => {
                if expected.get(*next) != Some(&binding) {
                    return Err(TradingSbfError::Transition.into());
                }
                *next = next.checked_add(1).ok_or(TradingSbfError::Transition)?;
                Ok(())
            }
        }
    }

    fn collected(self) -> Result<Vec<PreparedImmutableIdentityBindingV4>, ProgramError> {
        match self {
            Self::Collect(output) => Ok(output),
            Self::Verify { .. } => Err(TradingSbfError::Transition.into()),
        }
    }

    fn exhausted(&self) -> Result<(), ProgramError> {
        match self {
            Self::Collect(_) => Err(TradingSbfError::Transition.into()),
            Self::Verify { expected, next } => {
                if *next == expected.len() {
                    Ok(())
                } else {
                    Err(TradingSbfError::Transition.into())
                }
            }
        }
    }
}

/// Rewrite one coordinate's planned lamport balance.
///
/// Only the balance moves while a lifecycle batch is planned, so the candidate
/// the next invocation reads is the authenticated observation bank under a
/// lamport overlay, and one planned invocation rewrites the two entries it
/// touches. Materialising a whole 90-coordinate observation bank per batch cost
/// 4,320 bytes of a 32,768-byte heap on an allocator that never frees, to carry
/// 720 bytes of balance and 3,600 bytes of exact duplicate.
fn set_candidate_lamports_v3(
    index: usize,
    value: u64,
    planned_lamports: &mut [u64],
) -> Result<(), ProgramError> {
    *planned_lamports
        .get_mut(index)
        .ok_or(TradingSbfError::Content)? = value;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
/// Every working bank one lifecycle preplan needs, allocated once.
///
/// `prepare_lifecycle_v4` runs twice for one execution: once from the
/// request-projected registers to give the transition its plan, and once from
/// the transition's outputs to prove the plan it saw is the plan its outputs
/// produce. The SBF allocator never frees, so a second pass otherwise charged a
/// fresh 90-coordinate planned-balance overlay, four register banks and a state
/// reservation bank against total-ever-allocated for a pass whose only purpose
/// is to agree with the first.
pub(super) struct LifecyclePreplanScratchV4<'region> {
    planned_lamports: ScratchVecV1<'region, u64>,
    scalar_scratch: ScratchVecV1<'region, u64>,
    identity_scratch: ScratchVecV1<'region, [u8; 32]>,
    pub(super) next_scalars: ScratchVecV1<'region, u64>,
    pub(super) next_identities: ScratchVecV1<'region, [u8; 32]>,
    used_states: ScratchVecV1<'region, bool>,
}

impl<'region> LifecyclePreplanScratchV4<'region> {
    /// Build the arena, renting the two register-bank pairs the request
    /// projection finished with instead of allocating two fresh ones.
    ///
    /// The planned-balance overlay starts at the authenticated balances, which
    /// is exactly the candidate state before any invocation is planned.
    ///
    /// The rented banks arrive holding whatever the projection rotation left in
    /// them, so they are zeroed here: this is the same initial state
    /// `vec![0; n]` produced, reached without asking an allocator that never
    /// frees for a second copy of a buffer that already exists.
    pub(super) fn new(
        region: &'region HeapScratchRegionV1,
        observations: &[AccountObservationV1<'_>],
        accounts: &[&AccountInfo<'_>],
        scalar_count: usize,
        identity_count: usize,
        spare_scalars: [ScratchVecV1<'region, u64>; 2],
        spare_identities: [ScratchVecV1<'region, [u8; 32]>; 2],
    ) -> Result<Box<Self>, ProgramError> {
        if observations.len() != accounts.len() {
            return Err(TradingSbfError::Content.into());
        }
        let [mut scalar_scratch, mut next_scalars] = spare_scalars;
        let [mut identity_scratch, mut next_identities] = spare_identities;
        if scalar_scratch.len() != scalar_count
            || next_scalars.len() != scalar_count
            || identity_scratch.len() != identity_count
            || next_identities.len() != identity_count
        {
            return Err(TradingSbfError::Content.into());
        }
        scalar_scratch.fill(0);
        next_scalars.fill(0);
        identity_scratch.fill([0_u8; 32]);
        next_identities.fill([0_u8; 32]);
        let mut planned_lamports = ScratchVecV1::with_capacity(region, observations.len())?;
        for observation in observations {
            planned_lamports.push(observation.lamports())?;
        }
        // Boxed, and boxed here rather than at the call site: seven register
        // and observation banks are 168 bytes of `Vec` headers, and
        // `process_hot_execution_v3` is close enough to the 4KB SBF frame limit
        // that carrying them as caller locals makes a later call overwrite the
        // frame. Behind one pointer the caller pays 8 bytes and the headers
        // live in this constructor's frame instead.
        Ok(Box::new(Self {
            planned_lamports,
            scalar_scratch,
            identity_scratch,
            next_scalars,
            next_identities,
            used_states: ScratchVecV1::filled(region, &false, observations.len())?,
        }))
    }
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_lifecycle_v4<'a, 'region>(
    program_id: &Pubkey,
    lifecycle_owner_program: &AccountInfo<'_>,
    expected_market: [u8; 32],
    expected_release_set: [u8; 32],
    expected_generation: u64,
    expected_rent_credit: [u8; 32],
    policy: StateLifecyclePolicyV5<'_>,
    action: u32,
    account_profile: AccountProfileV2<'_>,
    tail_count: u32,
    observations: &[AccountObservationV1<'a>],
    accounts: &[&AccountInfo<'_>],
    scalars: &[u64],
    identities: &[[u8; 32]],
    rent: &Rent,
    aliases: &[usize],
    profile_join: ValidatedProfileJoinV3<'_>,
    // The bumps the caller mined for the accounts this lifecycle CREATES, in
    // the order the plan reaches them. Consumed only by the preplan; the replan
    // reuses the preplan's table and derives nothing. A slot the caller left
    // zero, and every created account past the end of the block, searches.
    lifecycle_hints: [u8; 2],
    // `None` on the preplan, which collects the table. `Some` on the replan,
    // which reproduces it: see [`LifecycleBatchSinkV4`] for why the second
    // evaluation is not redundant and why it allocates nothing.
    expected: Option<&[PreparedLifecycleInvocationV3]>,
    scratch: &mut LifecyclePreplanScratchV4<'region>,
    // Rented, never allocated. Both preplan passes want a working copy of the
    // register banks they were handed, and on an allocator that never frees a
    // `to_vec()` per pass charges the heap two whole pairs for two copies that
    // are never live at the same time as the bank they came from.
    mut output_scalars: ScratchVecV1<'region, u64>,
    mut output_identities: ScratchVecV1<'region, [u8; 32]>,
) -> Result<PreparedLifecycleBatchV4<'region>, ProgramError> {
    if observations.len() != accounts.len()
        || aliases.len() != accounts.len()
        || scratch.planned_lamports.len() != accounts.len()
        || scratch.used_states.len() != accounts.len()
        || scratch.scalar_scratch.len() != scalars.len()
        || scratch.next_scalars.len() != scalars.len()
        || scratch.identity_scratch.len() != identities.len()
        || scratch.next_identities.len() != identities.len()
    {
        return Err(TradingSbfError::Content.into());
    }
    if output_scalars.len() != scalars.len() || output_identities.len() != identities.len() {
        return Err(TradingSbfError::Content.into());
    }
    output_scalars.copy_from_slice(scalars);
    output_identities.copy_from_slice(identities);
    // Every working bank is rented from one arena that outlives both passes.
    // The SBF allocator never frees, so a second preplan otherwise charged a
    // fresh 90-coordinate candidate bank, four register banks and a state
    // reservation bank against total-ever-allocated purely to agree with the
    // first. They are reset here rather than reallocated.
    let LifecyclePreplanScratchV4 {
        planned_lamports,
        scalar_scratch,
        identity_scratch,
        next_scalars,
        next_identities,
        used_states,
    } = scratch;
    used_states.fill(false);
    for (slot, observation) in planned_lamports.iter_mut().zip(observations) {
        *slot = observation.lamports();
    }
    let plan_count =
        hot_cu_watch_lifecycle!(policy.action_plan_count(action), 3, 0, 0, u64::from(action))
            .map_err(|_| TradingSbfError::Content)?;
    let mut planned = 0_usize;
    let mut counted = 0_u16;
    while counted < plan_count {
        planned = planned
            .checked_add(
                usize::try_from(
                    policy
                        .action_plan(action, counted)
                        .map_err(|_| TradingSbfError::Content)?
                        .invocation_count(tail_count)
                        .map_err(|_| TradingSbfError::Content)?,
                )
                .map_err(|_| TradingSbfError::Content)?,
            )
            .ok_or(TradingSbfError::Content)?;
        counted = counted.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    let mut sink = LifecycleBatchSinkV4::new(expected, planned)?;
    // Which hint slot the next created account takes. Both passes walk the same
    // plan in the same order, so the two walks agree on the assignment without
    // carrying it between them.
    let mut next_hint = 0_usize;
    let mut ordinal = 0_u16;
    while ordinal < plan_count {
        let selected = hot_cu_watch_lifecycle!(
            policy.action_plan(action, ordinal),
            4,
            u64::from(ordinal),
            0,
            0
        )
        .map_err(|_| TradingSbfError::Content)?
        .with_validated_join(profile_join);
        let invocation_count = hot_cu_watch_lifecycle!(
            selected.invocation_count(tail_count),
            5,
            u64::from(ordinal),
            0,
            u64::from(tail_count)
        )
        .map_err(|_| TradingSbfError::Content)?;
        let mut invocation = 0_u32;
        while invocation < invocation_count {
            let item = hot_cu_watch_lifecycle!(
                selected.invocation_item(tail_count, invocation),
                6,
                u64::from(ordinal),
                u64::from(invocation),
                0
            )
            .map_err(|_| TradingSbfError::Content)?;
            let registers = LifecycleRegistersV3 {
                scalars: &output_scalars,
                identities: &output_identities,
            };
            if !hot_cu_watch_lifecycle!(
                selected.is_enabled(account_profile, tail_count, item, registers),
                70,
                u64::from(ordinal),
                u64::from(invocation),
                0
            )
            .map_err(|_| TradingSbfError::Content)?
            {
                hot_cu_lifecycle_prepare!(71, u64::from(ordinal), u64::from(invocation), 0);
                return Err(TradingSbfError::Content.into());
            }
            let prior = hot_cu_watch_lifecycle!(
                sink.expected(),
                12,
                u64::from(ordinal),
                u64::from(invocation),
                0
            )?;
            let indices = hot_cu_watch_lifecycle!(
                selected.project_account_indices(account_profile, tail_count, item),
                8,
                u64::from(ordinal),
                u64::from(invocation),
                0
            )
            .map_err(|_| TradingSbfError::Content)?;
            let state = hot_cu_watch_lifecycle!(
                representative_v3(indices.state(), aliases),
                15,
                u64::from(ordinal),
                u64::from(invocation),
                indices.state() as u64
            )?;
            hot_cu_watch_lifecycle!(
                reserve_lifecycle_state_v3(state, used_states),
                16,
                u64::from(ordinal),
                u64::from(invocation),
                state as u64
            )?;
            let payer = indices
                .payer()
                .map(|index| representative_v3(index, aliases))
                .transpose()?;
            let rent_credit = indices
                .rent_credit()
                .map(|index| representative_v3(index, aliases))
                .transpose()?;

            let seed_count = hot_cu_watch_lifecycle!(
                selected.seed_count(),
                10,
                u64::from(ordinal),
                u64::from(invocation),
                0
            )
            .map_err(|_| TradingSbfError::Content)?;
            let mut seeds =
                LifecycleSeedsV4::new(prior.map(|prior| prior.seeds.as_slice()), seed_count)?;
            let mut derived = None;
            let mut canonical_bump = None;
            let mut seed = 0_u8;
            while seed < seed_count {
                match hot_cu_watch_lifecycle!(
                    selected.materialize_seed_input(
                        account_profile,
                        tail_count,
                        item,
                        registers,
                        seed
                    ),
                    11,
                    u64::from(ordinal),
                    u64::from(invocation),
                    u64::from(seed)
                )
                .map_err(|_| TradingSbfError::Content)?
                {
                    LifecycleSeedInputValueV3::Bytes(value) => {
                        if canonical_bump.is_some() {
                            return Err(TradingSbfError::Content.into());
                        }
                        seeds.push(value.as_slice())?;
                    }
                    LifecycleSeedInputValueV3::CanonicalBump => {
                        if seed.checked_add(1) != Some(seed_count) || canonical_bump.is_some() {
                            return Err(TradingSbfError::Content.into());
                        }
                        let hint = lifecycle_hints.get(next_hint).copied().unwrap_or(0);
                        next_hint = next_hint.checked_add(1).ok_or(TradingSbfError::Content)?;
                        let bump = match hot_cu_watch_lifecycle!(
                            seeds.pending_bump(program_id, hint),
                            20,
                            u64::from(ordinal),
                            u64::from(invocation),
                            u64::from(hint)
                        )? {
                            LifecycleCanonicalBumpV4::Derived { address, bump } => {
                                derived = Some(address);
                                bump
                            }
                            LifecycleCanonicalBumpV4::Reused { bump } => {
                                // The preplan derived this address from these
                                // exact seed bytes and checked it against this
                                // exact account; `admit` refuses below unless
                                // the state coordinate is the preplan's too.
                                derived =
                                    Some(*accounts.get(state).ok_or(TradingSbfError::Content)?.key);
                                bump
                            }
                        };
                        seeds.push(&[bump])?;
                        canonical_bump = Some(bump);
                    }
                }
                seed = seed.checked_add(1).ok_or(TradingSbfError::Content)?;
            }
            let derived = derived.ok_or(TradingSbfError::Content)?;
            let canonical_bump = canonical_bump.ok_or(TradingSbfError::Content)?;
            if accounts
                .get(state)
                .is_none_or(|account| account.key != &derived)
            {
                hot_cu_lifecycle_prepare!(
                    14,
                    u64::from(ordinal),
                    u64::from(invocation),
                    state as u64
                );
                hot_cu_lifecycle_prepare!(
                    140,
                    u64::from_le_bytes(derived.to_bytes()[..8].try_into().unwrap_or([0; 8])),
                    accounts.get(state).map_or(0, |account| u64::from_le_bytes(
                        account.key.to_bytes()[..8].try_into().unwrap_or([0; 8])
                    )),
                    u64::from(canonical_bump)
                );
                return Err(TradingSbfError::Content.into());
            }
            let authenticated_credit = rent_credit
                .map(|index| {
                    hot_cu_watch_lifecycle!(
                        authenticate_lifecycle_credit_v3(
                            accounts,
                            lifecycle_owner_program,
                            index,
                            *planned_lamports
                                .get(index)
                                .ok_or(TradingSbfError::Content)?,
                            expected_market,
                            expected_release_set,
                            expected_generation,
                            expected_rent_credit,
                        ),
                        26,
                        u64::from(ordinal),
                        u64::from(invocation),
                        index as u64
                    )
                })
                .transpose()?;
            let current_rent_minimum = if matches!(
                selected.operation(),
                LifecycleOperationV3::Create | LifecycleOperationV3::AuthenticateOrCreate
            ) {
                let data_bytes = hot_cu_watch_lifecycle!(
                    selected.target_data_bytes(tail_count),
                    27,
                    u64::from(ordinal),
                    u64::from(invocation),
                    0
                )
                .map_err(|_| TradingSbfError::Content)?;
                Some(AuthenticatedRentMinimumV3 {
                    data_bytes,
                    lamports: rent.minimum_balance(
                        usize::try_from(data_bytes).map_err(|_| TradingSbfError::Content)?,
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
                    account_profile,
                    tail_count,
                    item_index: item,
                    accounts: hot_cu_watch_lifecycle!(
                        PlannedObservationsV3::planned(observations, planned_lamports),
                        29,
                        u64::from(ordinal),
                        u64::from(invocation),
                        0
                    )
                    .map_err(|_| TradingSbfError::Content)?,
                    registers: LifecycleRegistersV3 {
                        scalars: &output_scalars,
                        identities: &output_identities,
                    },
                    trading_program: program_id.to_bytes(),
                    system_program: system_program::ID.to_bytes(),
                    adapter_derived_pda: derived.to_bytes(),
                    rent_credit: authenticated_credit,
                    current_rent_minimum,
                },
                canonical_bump,
                LifecycleProtectedRegisterBuffersV3 {
                    scalar_scratch,
                    identity_scratch,
                    output_scalars: next_scalars,
                    output_identities: next_identities,
                },
            );
            let plan = hot_cu_watch_lifecycle!(
                plan,
                30,
                u64::from(ordinal),
                u64::from(invocation),
                state as u64
            )
            .map_err(|_| TradingSbfError::Content)?;
            if state == 0 && !matches!(plan, StateLifecyclePlanV3::Close(_)) {
                hot_cu_lifecycle_prepare!(31, u64::from(ordinal), u64::from(invocation), 0);
                return Err(TradingSbfError::Content.into());
            }
            let binding_count = hot_cu_watch_lifecycle!(
                selected.immutable_identity_binding_count(),
                32,
                u64::from(ordinal),
                u64::from(invocation),
                0
            )
            .map_err(|_| TradingSbfError::Content)?;
            let mut immutable_identity_bindings = LifecycleBindingsV4::new(
                prior.map(|prior| prior.immutable_identity_bindings.as_slice()),
                binding_count,
            )?;
            let absorbed = absorb_immutable_identity_bindings_v4(
                selected,
                account_profile,
                item,
                next_identities,
                binding_count,
                &mut immutable_identity_bindings,
            );
            hot_cu_watch_lifecycle!(
                absorbed,
                33,
                u64::from(ordinal),
                u64::from(invocation),
                u64::from(binding_count)
            )?;
            match plan {
                StateLifecyclePlanV3::Authenticate(_) => {}
                StateLifecyclePlanV3::Create(value) => {
                    for (index, balance) in [
                        (state, value.state_after),
                        (payer.ok_or(TradingSbfError::Content)?, value.payer_after),
                    ] {
                        set_candidate_lamports_v3(index, balance, planned_lamports)?;
                    }
                }
                StateLifecyclePlanV3::Close(value) => {
                    for (index, balance) in [
                        (state, value.source_after),
                        (
                            rent_credit.ok_or(TradingSbfError::Content)?,
                            value.rent_credit_after,
                        ),
                    ] {
                        set_candidate_lamports_v3(index, balance, planned_lamports)?;
                    }
                }
            }
            sink.admit(
                plan,
                state,
                payer,
                rent_credit,
                seeds,
                immutable_identity_bindings,
            )?;
            output_scalars.copy_from_slice(next_scalars);
            output_identities.copy_from_slice(next_identities);
            invocation = invocation.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(PreparedLifecycleBatchV4 {
        plans: sink.finish(planned)?,
        scalars: output_scalars,
        identities: output_identities,
    })
}

/// Materialize one invocation's immutable identity bindings into `output`.
///
/// Streaming rather than returning a vector: the replan's `output` compares
/// each binding against the preplan's and keeps nothing, so on the second pass
/// this allocates zero.
fn absorb_immutable_identity_bindings_v4(
    selected: dclutch_vm::account_profile::lifecycle_v3::SelectedLifecycleV3<'_>,
    profile: AccountProfileV2<'_>,
    item: Option<u32>,
    identities: &[[u8; 32]],
    count: u16,
    output: &mut LifecycleBindingsV4<'_>,
) -> Result<(), ProgramError> {
    let mut ordinal = 0_u16;
    while ordinal < count {
        let binding = selected
            .immutable_identity_binding(ordinal)
            .map_err(|_| TradingSbfError::Content)?;
        let target = binding.canonical();
        if target.kind() != LifecycleRegisterKindV3::Identity {
            return Err(TradingSbfError::Content.into());
        }
        let index = match target.scope() {
            CoordinateScopeV3::Fixed => usize::from(target.index()),
            CoordinateScopeV3::Item => usize::from(profile.common_identity_count())
                .checked_add(
                    usize::try_from(item.ok_or(TradingSbfError::Content)?)
                        .map_err(|_| TradingSbfError::Content)?
                        .checked_mul(usize::from(profile.item_identity_stride()))
                        .ok_or(TradingSbfError::Content)?,
                )
                .and_then(|base| base.checked_add(usize::from(target.index())))
                .ok_or(TradingSbfError::Content)?,
        };
        let canonical = *identities.get(index).ok_or(TradingSbfError::Content)?;
        if canonical == [0; 32] {
            return Err(TradingSbfError::Content.into());
        }
        output.push(PreparedImmutableIdentityBindingV4 {
            data_offset: binding.data_offset(),
            canonical,
        })?;
        ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(())
}

/// Require the transition's own outputs to be a fixed point of the lifecycle
/// projection.
///
/// The other half of the agreement - that the replan reproduces the preplan's
/// plan table - is decided invocation by invocation inside the replan itself,
/// at [`LifecycleBatchSinkV4::admit`], so it refuses at the first divergence
/// instead of after building a whole second table to compare. What is left here
/// is the half that is about the transition rather than the plan: the registers
/// the replan projects out of the transition's outputs must be those outputs.
///
/// The preplan's own register banks are rented out to the replan by the time
/// this runs and were never part of this agreement.
pub(super) fn require_lifecycle_replan_agreement_v4(
    revalidated_scalars: &[u64],
    revalidated_identities: &[[u8; 32]],
    transition_scalars: &[u64],
    transition_identities: &[[u8; 32]],
) -> Result<(), ProgramError> {
    if revalidated_scalars != transition_scalars || revalidated_identities != transition_identities
    {
        Err(TradingSbfError::Transition.into())
    } else {
        Ok(())
    }
}

/// Apply the three local-effect predicates to ONE resolved Effect operation:
/// leave the root header alone, mark each created state's immutable identity
/// binding it writes, and record which representative it mutates.
///
/// There is now exactly one walk. `project_atomic_visiting` offers each
/// operation to this function after the runtime-write overlap refusal has
/// accepted it and before the projection applies it, so these predicates ride
/// the walk the projection was making anyway.
///
/// The scan is over the Effect, not over the bindings: there are far more
/// operations than bindings, and asking each binding separately re-resolved the
/// entire program once per binding.
///
/// Each operation is checked completely before the next is resolved, so an
/// operation that both writes the root header and collides with a binding
/// refuses on the root header. Neither refusal is reachable-only-after the
/// other; there is no precedence to preserve between two operations, and both
/// are fail-closed.
///
/// `plans` is the **preplan's** table rather than the replan's. The replan
/// agreement that follows proves the two are equal, so answering binding
/// coverage against one and register identity against the other is the same
/// conjunction written in the other order - and the agreement still refuses
/// first-class if they ever disagree.
///
/// `participation` is `None` when the Effect declares no child route, which is
/// precisely when route disjointness has no consumer and the local plan is the
/// only thing that can move a lamport.
pub(super) fn inspect_local_effect_discipline_v5(
    plans: &[PreparedLifecycleInvocationV3],
    resolved: ResolvedEffectV3,
    aliases: &[usize],
    written: &mut [bool],
    participation: Option<&mut [CoordinateParticipationV3]>,
) -> Result<(), ProgramError> {
    require_root_write_is_state_only(resolved, aliases)?;
    if selected_root_lifecycle_close_v3(plans)? {
        require_no_root_local_mutation_v3(resolved, aliases)?;
    }
    inspect_lifecycle_binding_effects_v4(plans, resolved, aliases, written)?;
    if let Some(bank) = participation {
        mark_local_mutation(resolved, aliases, bank)?;
    }
    Ok(())
}

pub(super) fn require_no_root_local_mutation_v3(
    resolved: ResolvedEffectV3,
    aliases: &[usize],
) -> Result<(), ProgramError> {
    let coordinates = match resolved {
        ResolvedEffectV3::TransferLamports {
            source,
            destination,
            ..
        } => [Some(source), Some(destination)],
        ResolvedEffectV3::WriteScalar { account, .. }
        | ResolvedEffectV3::WriteIdentity { account, .. }
        | ResolvedEffectV3::WriteU8 { account, .. }
        | ResolvedEffectV3::WriteU16 { account, .. }
        | ResolvedEffectV3::WriteU32 { account, .. } => [Some(account), None],
        ResolvedEffectV3::Noop
        | ResolvedEffectV3::RequireLamportsEq { .. }
        | ResolvedEffectV3::WriteRequest { .. } => [None, None],
    };
    for coordinate in coordinates.into_iter().flatten() {
        if representative_v3(coordinate, aliases)? == 0 {
            return Err(TradingSbfError::Transition.into());
        }
    }
    Ok(())
}

/// Require every created state's immutable identity binding to have been
/// written by exactly one of the operations the walk offered.
///
/// This is the only half of the discipline that cannot ride an operation: it is
/// a fact about the whole program, answered once the walk has ended.
pub(super) fn require_lifecycle_binding_coverage_v4(
    plans: &[PreparedLifecycleInvocationV3],
    written: &[bool],
) -> Result<(), ProgramError> {
    let mut ordinal = 0_usize;
    for prepared in plans {
        for _ in &prepared.immutable_identity_bindings {
            if matches!(prepared.plan, StateLifecyclePlanV3::Create(_))
                && written.get(ordinal) != Some(&true)
            {
                return Err(TradingSbfError::Transition.into());
            }
            ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Transition)?;
        }
    }
    Ok(())
}

/// Fold one resolved Effect write against every planned binding.
pub(super) fn inspect_lifecycle_binding_effects_v4(
    plans: &[PreparedLifecycleInvocationV3],
    resolved: ResolvedEffectV3,
    aliases: &[usize],
    written: &mut [bool],
) -> Result<(), ProgramError> {
    let mut ordinal = 0_usize;
    for prepared in plans {
        for binding in &prepared.immutable_identity_bindings {
            let flag = written
                .get_mut(ordinal)
                .ok_or(TradingSbfError::Transition)?;
            *flag |=
                inspect_lifecycle_binding_effect_v4(prepared.state, binding, resolved, aliases)?;
            ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Transition)?;
        }
    }
    Ok(())
}

pub(super) fn inspect_lifecycle_binding_effect_v4(
    state: usize,
    binding: &PreparedImmutableIdentityBindingV4,
    effect: ResolvedEffectV3,
    aliases: &[usize],
) -> Result<bool, ProgramError> {
    let (account, offset, width, identity) = match effect {
        ResolvedEffectV3::WriteScalar {
            account, offset, ..
        } => (account, offset, 8_u32, None),
        ResolvedEffectV3::WriteIdentity {
            account,
            offset,
            value,
        } => (account, offset, 32_u32, Some(value)),
        ResolvedEffectV3::WriteU8 {
            account, offset, ..
        } => (account, offset, 1_u32, None),
        ResolvedEffectV3::WriteU16 {
            account, offset, ..
        } => (account, offset, 2_u32, None),
        ResolvedEffectV3::WriteU32 {
            account, offset, ..
        } => (account, offset, 4_u32, None),
        ResolvedEffectV3::Noop
        | ResolvedEffectV3::TransferLamports { .. }
        | ResolvedEffectV3::RequireLamportsEq { .. }
        | ResolvedEffectV3::WriteRequest { .. } => return Ok(false),
    };
    if representative_v3(account, aliases)? != state
        || !ranges_overlap_v4(offset, width, binding.data_offset, 32)?
    {
        return Ok(false);
    }
    if offset == binding.data_offset && identity == Some(binding.canonical) {
        Ok(true)
    } else {
        Err(TradingSbfError::Transition.into())
    }
}

fn ranges_overlap_v4(
    left_start: u32,
    left_width: u32,
    right_start: u32,
    right_width: u32,
) -> Result<bool, ProgramError> {
    let left_end = left_start
        .checked_add(left_width)
        .ok_or(TradingSbfError::Transition)?;
    let right_end = right_start
        .checked_add(right_width)
        .ok_or(TradingSbfError::Transition)?;
    Ok(left_start < right_end && right_start < left_end)
}

#[cfg(test)]
pub(super) fn require_canonical_lifecycle_pda_v3(
    program_id: &Pubkey,
    seed_slices: &[&[u8]],
) -> Result<Pubkey, ProgramError> {
    let (bump_seed, canonical_seeds) = seed_slices.split_last().ok_or(TradingSbfError::Content)?;
    let [supplied_bump] = bump_seed else {
        return Err(TradingSbfError::Content.into());
    };
    let (derived, canonical_bump) = Pubkey::try_find_program_address(canonical_seeds, program_id)
        .ok_or(TradingSbfError::Content)?;
    if *supplied_bump != canonical_bump {
        return Err(TradingSbfError::Content.into());
    }
    Ok(derived)
}

pub(super) fn representative_v3(index: usize, aliases: &[usize]) -> Result<usize, ProgramError> {
    aliases
        .get(index)
        .copied()
        .ok_or_else(|| TradingSbfError::Content.into())
}

pub(super) fn reserve_lifecycle_state_v3(
    state: usize,
    used_states: &mut [bool],
) -> Result<(), ProgramError> {
    if used_states
        .get(state)
        .copied()
        .ok_or(TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    *used_states.get_mut(state).ok_or(TradingSbfError::Content)? = true;
    Ok(())
}

pub(super) fn authenticate_lifecycle_credit_v3(
    accounts: &[&AccountInfo<'_>],
    owner_program: &AccountInfo<'_>,
    index: usize,
    observed_lamports: u64,
    expected_market: [u8; 32],
    expected_release_set: [u8; 32],
    expected_generation: u64,
    expected_key: [u8; 32],
) -> Result<AuthenticatedRentCreditV3, ProgramError> {
    let account = accounts.get(index).ok_or(TradingSbfError::Content)?;
    // The credit's persisted owner identifies the program whose PDA namespace
    // owns this lifecycle credit, and it is PROVEN BELOW, not pinned here: the
    // `create_program_address` at the end of this function re-derives this
    // account's own key from the credit's own seeds under `account.owner`, so
    // an owner that did not mint this credit cannot reproduce its address.
    //
    // There is no fixed rent-program coordinate to swap in: `frame.rent` is the
    // rent SYSVAR (`sysvar::rent::ID`), and `ExecutionRoleV1` pins only Core,
    // Claims, Trading, Resolution and Custody. Pinning Rent as a release role
    // is the honest destination if this is ever wanted as a fixed coordinate;
    // until then the derivation below is the stronger check anyway, because it
    // binds the owner to THIS credit rather than to a list of admissible ones.
    if account.key.to_bytes() != expected_key
        || owner_program.is_signer
        || owner_program.is_writable
        || !owner_program.executable
        || account.is_signer
        || !account.is_writable
        || account.executable
        || account.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || !funded_rent_persists_v1(observed_lamports)
    {
        return Err(TradingSbfError::Content.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let credit = LifecycleRentCreditV2::decode(&data).map_err(|_| TradingSbfError::Content)?;
    if credit.to_bytes().as_slice() != data.as_ref()
        || credit.market().to_bytes() != expected_market
        || credit.release_set().to_bytes() != expected_release_set
        || credit.generation() != expected_generation
    {
        return Err(TradingSbfError::Content.into());
    }
    let seeds = credit.pda_seeds();
    let authority = credit.refund_wallet().to_bytes();
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market.as_slice(),
            generation.as_slice(),
            &bump,
        ],
        account.owner,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if account.key != &expected {
        return Err(TradingSbfError::Content.into());
    }
    Ok(AuthenticatedRentCreditV3 {
        key: account.key.to_bytes(),
        beneficiary: authority,
        lamports: observed_lamports,
    })
}

pub(super) fn apply_lifecycle_candidates_v3(
    plans: &[PreparedLifecycleInvocationV3],
    aliases: &[usize],
    accounts: &mut [AccountInput],
) -> Result<(), ProgramError> {
    for prepared in plans {
        match prepared.plan {
            StateLifecyclePlanV3::Authenticate(_) => {}
            StateLifecyclePlanV3::Create(plan) => {
                set_account_candidate_v3(
                    prepared.state,
                    aliases,
                    accounts,
                    plan.state_after,
                    usize::try_from(plan.target_data_bytes)
                        .map_err(|_| TradingSbfError::Content)?,
                )?;
                set_account_candidate_lamports_v3(
                    prepared.payer.ok_or(TradingSbfError::Content)?,
                    aliases,
                    accounts,
                    plan.payer_after,
                )?;
            }
            StateLifecyclePlanV3::Close(plan) => {
                set_account_candidate_v3(prepared.state, aliases, accounts, plan.source_after, 0)?;
                set_account_candidate_lamports_v3(
                    prepared.rent_credit.ok_or(TradingSbfError::Content)?,
                    aliases,
                    accounts,
                    plan.rent_credit_after,
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn apply_funding_candidates_v5(
    effect: Option<EffectProgramV5<'_>>,
    scalars: &[u64],
    aliases: &[usize],
    accounts: &mut [AccountInput],
) -> Result<(), ProgramError> {
    let Some(effect) = effect else {
        return Ok(());
    };
    let mut index = 0_u16;
    while index < effect.funding_action_count() {
        let action = effect
            .funding_action(index)
            .map_err(|_| TradingSbfError::Transition)?;
        let state = usize::from(action.state());
        let counterparty = usize::from(action.counterparty());
        if aliases.get(state).copied() != Some(state)
            || aliases.get(counterparty).copied() != Some(counterparty)
        {
            return Err(TradingSbfError::Transition.into());
        }
        let amount = *scalars
            .get(usize::from(action.lamports_scalar()))
            .ok_or(TradingSbfError::Transition)?;
        match action.operation() {
            FundingOperationV5::Create => {
                let refund = usize::from(
                    action
                        .refund_destination()
                        .ok_or(TradingSbfError::Transition)?,
                );
                if aliases.get(refund).copied() != Some(refund) {
                    return Err(TradingSbfError::Transition.into());
                }
                let state_before = accounts
                    .get(state)
                    .ok_or(TradingSbfError::Transition)?
                    .lamports;
                if state_before < amount {
                    let debit = amount
                        .checked_sub(state_before)
                        .ok_or(TradingSbfError::Transition)?;
                    let payer = accounts
                        .get_mut(counterparty)
                        .ok_or(TradingSbfError::Transition)?;
                    payer.lamports = payer
                        .lamports
                        .checked_sub(debit)
                        .ok_or(TradingSbfError::Transition)?;
                } else if state_before > amount {
                    let surplus = state_before
                        .checked_sub(amount)
                        .ok_or(TradingSbfError::Transition)?;
                    let refund = accounts
                        .get_mut(refund)
                        .ok_or(TradingSbfError::Transition)?;
                    refund.lamports = refund
                        .lamports
                        .checked_add(surplus)
                        .ok_or(TradingSbfError::Transition)?;
                }
                let state = accounts.get_mut(state).ok_or(TradingSbfError::Transition)?;
                state.lamports = amount;
                state.data_len = usize::try_from(action.live_bytes())
                    .map_err(|_| TradingSbfError::Transition)?;
            }
            FundingOperationV5::Close => {
                let credit = accounts
                    .get_mut(counterparty)
                    .ok_or(TradingSbfError::Transition)?;
                credit.lamports = credit
                    .lamports
                    .checked_add(amount)
                    .ok_or(TradingSbfError::Transition)?;
                let state = accounts.get_mut(state).ok_or(TradingSbfError::Transition)?;
                state.lamports = 0;
                state.data_len = 0;
            }
        }
        index = index.checked_add(1).ok_or(TradingSbfError::Transition)?;
    }
    Ok(())
}

fn set_account_candidate_v3(
    representative: usize,
    aliases: &[usize],
    accounts: &mut [AccountInput],
    lamports: u64,
    data_len: usize,
) -> Result<(), ProgramError> {
    for (coordinate, alias) in aliases.iter().enumerate() {
        if *alias == representative {
            let account = accounts
                .get_mut(coordinate)
                .ok_or(TradingSbfError::Content)?;
            account.lamports = lamports;
            account.data_len = data_len;
        }
    }
    Ok(())
}

fn set_account_candidate_lamports_v3(
    representative: usize,
    aliases: &[usize],
    accounts: &mut [AccountInput],
    lamports: u64,
) -> Result<(), ProgramError> {
    for (coordinate, alias) in aliases.iter().enumerate() {
        if *alias == representative {
            accounts
                .get_mut(coordinate)
                .ok_or(TradingSbfError::Content)?
                .lamports = lamports;
        }
    }
    Ok(())
}

pub(super) fn apply_lifecycle_creates_v3(
    program_id: &Pubkey,
    plans: &[PreparedLifecycleInvocationV3],
    accounts: &[&AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let system = accounts
        .iter()
        .find(|account| {
            account.key == &system_program::ID
                && account.executable
                && !account.is_signer
                && !account.is_writable
        })
        .copied();
    for prepared in plans {
        let StateLifecyclePlanV3::Create(plan) = prepared.plan else {
            continue;
        };
        let system = system.ok_or(TradingSbfError::Commit)?;
        let state = accounts
            .get(prepared.state)
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let payer = accounts
            .get(prepared.payer.ok_or(TradingSbfError::Commit)?)
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        if state.key.to_bytes() != plan.state
            || payer.key.to_bytes() != plan.payer
            || state.owner != &system_program::ID
            || state.data_len() != 0
            || state.lamports() != plan.state_before
            || payer.lamports()
                != plan
                    .payer_after
                    .checked_add(plan.payer_debit)
                    .ok_or(TradingSbfError::Commit)?
        {
            return Err(TradingSbfError::Commit.into());
        }
        if plan.payer_debit != 0 {
            invoke(
                &system_transfer(payer.key, state.key, plan.payer_debit),
                &[payer.clone(), state.clone(), system.clone()],
            )
            .map_err(|_| TradingSbfError::Commit)?;
        }
        let seed_slices = prepared.seeds.iter().map(Vec::as_slice).collect::<Vec<_>>();
        invoke_signed(
            &allocate(state.key, u64::from(plan.target_data_bytes)),
            &[state.clone(), system.clone()],
            &[seed_slices.as_slice()],
        )
        .map_err(|_| TradingSbfError::Commit)?;
        invoke_signed(
            &assign(state.key, program_id),
            &[state.clone(), system.clone()],
            &[seed_slices.as_slice()],
        )
        .map_err(|_| TradingSbfError::Commit)?;
        let data = state
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Commit)?;
        if state.owner != program_id
            || state.lamports() != plan.state_after
            || data.len()
                != usize::try_from(plan.target_data_bytes).map_err(|_| TradingSbfError::Commit)?
            || data.iter().any(|byte| *byte != 0)
            || payer.lamports() != plan.payer_after
        {
            return Err(TradingSbfError::Commit.into());
        }
    }
    Ok(())
}

pub(super) fn apply_funding_creates_v5(
    program_id: &Pubkey,
    effect: Option<EffectProgramV5<'_>>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    accounts: &[&AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let Some(effect) = effect else {
        return Ok(());
    };
    let mut index = 0_u16;
    while index < effect.funding_action_count() {
        let action = effect
            .funding_action(index)
            .map_err(|_| TradingSbfError::Commit)?;
        if action.operation() != FundingOperationV5::Create {
            index = index.checked_add(1).ok_or(TradingSbfError::Commit)?;
            continue;
        }
        let state = accounts
            .get(usize::from(action.state()))
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let payer = accounts
            .get(usize::from(action.payer().ok_or(TradingSbfError::Commit)?))
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let refund = accounts
            .get(usize::from(
                action.refund_destination().ok_or(TradingSbfError::Commit)?,
            ))
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let system = accounts
            .get(usize::from(
                action.system_program().ok_or(TradingSbfError::Commit)?,
            ))
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let target = *scalars
            .get(usize::from(action.lamports_scalar()))
            .ok_or(TradingSbfError::Commit)?;
        let refund_owner = *identities
            .get(usize::from(action.refund_owner_identity()))
            .ok_or(TradingSbfError::Commit)?;
        let signer =
            prepare_funding_signer_v5(program_id, effect, index, tail_count, scalars, identities)?;
        if state.key != &signer.address
            || state.owner != &system_program::ID
            || state.data_len() != 0
            || !state.is_writable
            || state.is_signer
            || !payer.is_signer
            || !payer.is_writable
            || !refund.is_writable
            || refund.key.to_bytes() != refund_owner
            || system.key != &system_program::ID
            || !system.executable
        {
            return Err(TradingSbfError::Commit.into());
        }
        let state_before = state.lamports();
        let payer_before = payer.lamports();
        let refund_before = refund.lamports();
        let (payer_after, refund_after) = if state_before < target {
            let debit = target
                .checked_sub(state_before)
                .ok_or(TradingSbfError::Commit)?;
            let payer_after = payer_before
                .checked_sub(debit)
                .ok_or(TradingSbfError::Commit)?;
            invoke(
                &system_transfer(payer.key, state.key, debit),
                &[payer.clone(), state.clone(), system.clone()],
            )
            .map_err(|_| TradingSbfError::Commit)?;
            (payer_after, refund_before)
        } else if state_before > target {
            let surplus = state_before
                .checked_sub(target)
                .ok_or(TradingSbfError::Commit)?;
            let refund_after = refund_before
                .checked_add(surplus)
                .ok_or(TradingSbfError::Commit)?;
            invoke_funding_signed_v5(
                &signer,
                &system_transfer(state.key, refund.key, surplus),
                &[state.clone(), refund.clone(), system.clone()],
            )?;
            (payer_before, refund_after)
        } else {
            (payer_before, refund_before)
        };
        invoke_funding_signed_v5(
            &signer,
            &allocate(state.key, u64::from(action.live_bytes())),
            &[state.clone(), system.clone()],
        )?;
        invoke_funding_signed_v5(
            &signer,
            &assign(state.key, program_id),
            &[state.clone(), system.clone()],
        )?;
        let data = state
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Commit)?;
        if state.owner != program_id
            || state.lamports() != target
            || data.len()
                != usize::try_from(action.live_bytes()).map_err(|_| TradingSbfError::Commit)?
            || data.iter().any(|value| *value != 0)
            || payer.lamports() != payer_after
            || refund.lamports() != refund_after
        {
            return Err(TradingSbfError::Commit.into());
        }
        index = index.checked_add(1).ok_or(TradingSbfError::Commit)?;
    }
    Ok(())
}

fn invoke_funding_signed_v5(
    signer: &FundingSignerV5,
    instruction: &solana_program::instruction::Instruction,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let mut slices: [&[u8]; MAX_ACTION_SEEDS_V5 as usize] = [&[]; MAX_ACTION_SEEDS_V5 as usize];
    let mut index = 0_usize;
    while index < signer.non_bump {
        slices[index] = &signer.values[index][..signer.lengths[index]];
        index = index.checked_add(1).ok_or(TradingSbfError::Commit)?;
    }
    slices[signer.non_bump] = &signer.bump;
    invoke_signed(instruction, accounts, &[&slices[..=signer.non_bump]])
        .map_err(|_| TradingSbfError::Commit.into())
}

/// Whether a Close plan's recorded refund identity is admissible at the
/// mutation boundary.
///
/// This used to be `authenticated_credit.beneficiary == plan.beneficiary`, and
/// under a `Credit` plan it still is: a market-shared state's refund identity
/// is knowable from the credit at close time, so the boundary re-derives it and
/// refuses a plan naming anyone else.
///
/// A `Payer` plan's identity is not re-derivable here -- the payer funded the
/// state in some earlier transaction and is not an account of this one. The
/// kernel took that identity from the closing state's OWN bytes, an
/// AccountProfile projection rather than caller input, so what this boundary
/// can still own is that a create actually recorded one. The offset those bytes
/// live at belongs to the family, and the family's artifacts check it there.
fn closing_refund_identity_admitted_v3(
    plan: CloseStatePlanV3,
    credit_beneficiary: [u8; 32],
) -> bool {
    match plan.refund_source {
        LifecycleRefundSourceV3::Credit => credit_beneficiary == plan.beneficiary,
        LifecycleRefundSourceV3::Payer => plan.beneficiary != [0; 32],
    }
}

pub(super) fn apply_lifecycle_closes_v3(
    program_id: &Pubkey,
    lifecycle_owner_program: &AccountInfo<'_>,
    expected_market: [u8; 32],
    expected_release_set: [u8; 32],
    expected_generation: u64,
    expected_rent_credit: [u8; 32],
    plans: &[PreparedLifecycleInvocationV3],
    accounts: &[&AccountInfo<'_>],
) -> Result<(), ProgramError> {
    for prepared in plans {
        let StateLifecyclePlanV3::Close(plan) = prepared.plan else {
            continue;
        };
        let state = accounts
            .get(prepared.state)
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let credit = accounts
            .get(prepared.rent_credit.ok_or(TradingSbfError::Commit)?)
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let authenticated_credit = authenticate_lifecycle_credit_v3(
            accounts,
            lifecycle_owner_program,
            prepared.rent_credit.ok_or(TradingSbfError::Commit)?,
            credit.lamports(),
            expected_market,
            expected_release_set,
            expected_generation,
            expected_rent_credit,
        )?;
        if state.key.to_bytes() != plan.state
            || credit.key.to_bytes() != plan.rent_credit
            || state.owner != program_id
            || state.data_len()
                != usize::try_from(plan.source_data_bytes).map_err(|_| TradingSbfError::Commit)?
            || state.lamports() != plan.source_before
            || credit.lamports() != plan.rent_credit_before
            || !closing_refund_identity_admitted_v3(plan, authenticated_credit.beneficiary)
        {
            return Err(TradingSbfError::Commit.into());
        }
        state
            .try_borrow_mut_data()
            .map_err(|_| TradingSbfError::Commit)?
            .fill(0);
        **state
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)? = plan.source_after;
        **credit
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)? = plan.rent_credit_after;
        state.resize(0).map_err(|_| TradingSbfError::Commit)?;
        state.assign(&system_program::ID);
        if state.owner != &system_program::ID
            || state.data_len() != 0
            || state.lamports() != 0
            || credit.lamports() != plan.rent_credit_after
        {
            return Err(TradingSbfError::Commit.into());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_funding_closes_v5(
    program_id: &Pubkey,
    lifecycle_owner_program: &AccountInfo<'_>,
    effect: Option<EffectProgramV5<'_>>,
    profile: Option<AccountProfileV3<'_>>,
    _tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    accounts: &[&AccountInfo<'_>],
    market: [u8; 32],
    release_set: [u8; 32],
    generation: u64,
    expected_rent_credit: [u8; 32],
) -> Result<(), ProgramError> {
    let (Some(effect), Some(profile)) = (effect, profile) else {
        return if effect.is_none() && profile.is_none() {
            Ok(())
        } else {
            Err(TradingSbfError::Commit.into())
        };
    };
    let mut index = 0_u16;
    while index < effect.funding_action_count() {
        let action = effect
            .funding_action(index)
            .map_err(|_| TradingSbfError::Commit)?;
        if action.operation() != FundingOperationV5::Close {
            index = index.checked_add(1).ok_or(TradingSbfError::Commit)?;
            continue;
        }
        let state = accounts
            .get(usize::from(action.state()))
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let credit_index = usize::from(action.rent_credit().ok_or(TradingSbfError::Commit)?);
        let credit = accounts
            .get(credit_index)
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let bound = profile
            .funding_bound_for(action.state())
            .map_err(|_| TradingSbfError::Commit)?
            .ok_or(TradingSbfError::Commit)?;
        let observed = *scalars
            .get(usize::from(action.lamports_scalar()))
            .ok_or(TradingSbfError::Commit)?;
        let refund_owner = *identities
            .get(usize::from(action.refund_owner_identity()))
            .ok_or(TradingSbfError::Commit)?;
        let authenticated_credit = authenticate_lifecycle_credit_v3(
            accounts,
            lifecycle_owner_program,
            credit_index,
            credit.lamports(),
            market,
            release_set,
            generation,
            expected_rent_credit,
        )?;
        let credit_after = credit
            .lamports()
            .checked_add(observed)
            .ok_or(TradingSbfError::Commit)?;
        if state.owner != program_id
            || state.data_len()
                != usize::try_from(bound.live_bytes()).map_err(|_| TradingSbfError::Commit)?
            || state.lamports() != observed
            || authenticated_credit.beneficiary != refund_owner
            || !state.is_writable
            || !credit.is_writable
            || state.key == credit.key
        {
            return Err(TradingSbfError::Commit.into());
        }
        state
            .try_borrow_mut_data()
            .map_err(|_| TradingSbfError::Commit)?
            .fill(0);
        **state
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)? = 0;
        **credit
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)? = credit_after;
        state.resize(0).map_err(|_| TradingSbfError::Commit)?;
        state.assign(&system_program::ID);
        if state.owner != &system_program::ID
            || state.data_len() != 0
            || state.lamports() != 0
            || credit.lamports() != credit_after
        {
            return Err(TradingSbfError::Commit.into());
        }
        index = index.checked_add(1).ok_or(TradingSbfError::Commit)?;
    }
    Ok(())
}

/// A heap-allocated `T` whose allocation REFUSES instead of aborting.
///
/// `Box::new` allocates infallibly: on an exhausted heap it aborts the whole
/// invocation (`memory allocation failed` -> `ProgramFailedToComplete`), which
/// rolls the transaction back but names nothing and reaches no refusal code.
/// `Box::try_new` is unstable, so the fallible reservation goes through `Vec`,
/// whose `try_reserve_exact` is stable, and `into_boxed_slice` does not
/// reallocate at capacity equal to length. `Box<[T; 1]>` is that same
/// allocation with a safe conversion available; `Box<[T]> -> Box<T>` is not.
///
/// This exists because the phase-8 boundary is where the heap actually runs
/// out, and the composition decode is the first thing past it to allocate.
pub(super) struct HeapBoxV3<T>(Box<[T; 1]>);

impl<T> HeapBoxV3<T> {
    pub(super) fn new(value: T) -> Result<Self, ProgramError> {
        let mut storage = Vec::new();
        storage
            .try_reserve_exact(1)
            .map_err(|_| TradingSbfError::HeapExhausted)?;
        storage.push(value);
        Ok(Self(
            storage
                .into_boxed_slice()
                .try_into()
                .map_err(|_| TradingSbfError::Content)?,
        ))
    }
}

impl<T> core::ops::Deref for HeapBoxV3<T> {
    type Target = T;

    fn deref(&self) -> &T {
        let [value] = &*self.0;
        value
    }
}

impl<T> core::ops::DerefMut for HeapBoxV3<T> {
    fn deref_mut(&mut self) -> &mut T {
        let [value] = &mut *self.0;
        value
    }
}

pub(super) fn require_funding_profile_join_v5(
    effect: SelectedEffectProgramV4<'_>,
    profile: Option<AccountProfileV3<'_>>,
) -> Result<(), ProgramError> {
    let (Some(effect), Some(profile)) = (effect.funding(), profile) else {
        return if effect.funding().is_none() && profile.is_none() {
            Ok(())
        } else {
            Err(TradingSbfError::UnsupportedContent.into())
        };
    };
    let mut index = 0_u16;
    while index < effect.funding_action_count() {
        let action = effect
            .funding_action(index)
            .map_err(|_| TradingSbfError::Content)?;
        let bound = profile
            .funding_bound_for(action.state())
            .map_err(|_| TradingSbfError::Content)?
            .ok_or(TradingSbfError::Content)?;
        match action.operation() {
            FundingOperationV5::Create
                if bound.actions().permits_create()
                    && action.live_bytes() == bound.live_bytes() => {}
            FundingOperationV5::Close if bound.actions().permits_close() => {}
            _ => return Err(TradingSbfError::Content.into()),
        }
        index = index.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn require_funding_runtime_v5(
    program_id: &Pubkey,
    lifecycle_owner_program: &AccountInfo<'_>,
    effect: SelectedEffectProgramV4<'_>,
    profile: Option<AccountProfileV3<'_>>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    accounts: &[&AccountInfo<'_>],
    aliases: &[usize],
    rent: &Rent,
    market: [u8; 32],
    release_set: [u8; 32],
    generation: u64,
    expected_rent_credit: [u8; 32],
) -> Result<(), ProgramError> {
    let (Some(effect), Some(profile)) = (effect.funding(), profile) else {
        return if effect.funding().is_none() && profile.is_none() {
            Ok(())
        } else {
            Err(TradingSbfError::UnsupportedContent.into())
        };
    };
    let mut index = 0_u16;
    while index < effect.funding_action_count() {
        let action = effect
            .funding_action(index)
            .map_err(|_| TradingSbfError::Transition)?;
        let state_index = usize::from(action.state());
        let counterparty_index = usize::from(action.counterparty());
        require_funding_representative_v5(state_index, accounts, aliases)?;
        require_funding_representative_v5(counterparty_index, accounts, aliases)?;
        let state = accounts
            .get(state_index)
            .copied()
            .ok_or(TradingSbfError::Transition)?;
        let counterparty = accounts
            .get(counterparty_index)
            .copied()
            .ok_or(TradingSbfError::Transition)?;
        let refund_owner = *identities
            .get(usize::from(action.refund_owner_identity()))
            .ok_or(TradingSbfError::Transition)?;
        if refund_owner.iter().all(|value| *value == 0)
            || !state.is_writable
            || state.is_signer
            || state.executable
            || !counterparty.is_writable
            || counterparty.executable
        {
            return Err(TradingSbfError::Transition.into());
        }
        let amount = *scalars
            .get(usize::from(action.lamports_scalar()))
            .ok_or(TradingSbfError::Transition)?;
        match action.operation() {
            FundingOperationV5::Create => {
                let payer = counterparty;
                let refund_index = usize::from(
                    action
                        .refund_destination()
                        .ok_or(TradingSbfError::Transition)?,
                );
                let system_index =
                    usize::from(action.system_program().ok_or(TradingSbfError::Transition)?);
                require_funding_representative_v5(refund_index, accounts, aliases)?;
                require_funding_representative_v5(system_index, accounts, aliases)?;
                let refund = accounts
                    .get(refund_index)
                    .copied()
                    .ok_or(TradingSbfError::Transition)?;
                let system = accounts
                    .get(system_index)
                    .copied()
                    .ok_or(TradingSbfError::Transition)?;
                let live_bytes = usize::try_from(action.live_bytes())
                    .map_err(|_| TradingSbfError::Transition)?;
                let signer = prepare_funding_signer_v5(
                    program_id, effect, index, tail_count, scalars, identities,
                )?;
                if state.key != &signer.address
                    || state.owner != &system_program::ID
                    || state.data_len() != 0
                    || !payer.is_signer
                    || payer.key == state.key
                    || !refund.is_writable
                    || refund.is_signer
                    || refund.executable
                    || refund.key.to_bytes() != refund_owner
                    || system.key != &system_program::ID
                    || system.is_signer
                    || system.is_writable
                    || !system.executable
                    || amount != rent.minimum_balance(live_bytes)
                {
                    return Err(TradingSbfError::Transition.into());
                }
            }
            FundingOperationV5::Close => {
                let bound = profile
                    .funding_bound_for(action.state())
                    .map_err(|_| TradingSbfError::Transition)?
                    .ok_or(TradingSbfError::Transition)?;
                let live_bytes =
                    usize::try_from(bound.live_bytes()).map_err(|_| TradingSbfError::Transition)?;
                let authenticated_credit = authenticate_lifecycle_credit_v3(
                    accounts,
                    lifecycle_owner_program,
                    counterparty_index,
                    counterparty.lamports(),
                    market,
                    release_set,
                    generation,
                    expected_rent_credit,
                )?;
                if state.owner != program_id
                    || state.data_len() != live_bytes
                    || state.lamports() != amount
                    || state.key == counterparty.key
                    || counterparty.is_signer
                    || authenticated_credit.beneficiary != refund_owner
                {
                    return Err(TradingSbfError::Transition.into());
                }
            }
        }
        index = index.checked_add(1).ok_or(TradingSbfError::Transition)?;
    }
    require_funding_child_separation_v5(effect, tail_count, scalars, identities)
}

fn require_funding_representative_v5(
    coordinate: usize,
    accounts: &[&AccountInfo<'_>],
    aliases: &[usize],
) -> Result<(), ProgramError> {
    if coordinate == 0
        || coordinate >= accounts.len()
        || aliases.get(coordinate).copied() != Some(coordinate)
    {
        Err(TradingSbfError::Transition.into())
    } else {
        Ok(())
    }
}

struct FundingSignerV5 {
    values: [[u8; 32]; MAX_ACTION_SEEDS_V5 as usize],
    lengths: [usize; MAX_ACTION_SEEDS_V5 as usize],
    non_bump: usize,
    bump: [u8; 1],
    address: Pubkey,
}

fn prepare_funding_signer_v5(
    program_id: &Pubkey,
    effect: EffectProgramV5<'_>,
    action_index: u16,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<FundingSignerV5, ProgramError> {
    let action = effect
        .funding_action(action_index)
        .map_err(|_| TradingSbfError::Transition)?;
    if action.operation() != FundingOperationV5::Create || action.seed_count() < 1 {
        return Err(TradingSbfError::Transition.into());
    }
    let non_bump = usize::from(action.seed_count() - 1);
    if non_bump >= usize::from(MAX_ACTION_SEEDS_V5) {
        return Err(TradingSbfError::Transition.into());
    }
    let mut values = [[0_u8; 32]; MAX_ACTION_SEEDS_V5 as usize];
    let mut lengths = [0_usize; MAX_ACTION_SEEDS_V5 as usize];
    let mut ordinal = 0_usize;
    while ordinal < non_bump {
        let resolved = effect
            .resolve_funding_seed(
                action_index,
                u8::try_from(ordinal).map_err(|_| TradingSbfError::Transition)?,
                tail_count,
                scalars,
                identities,
            )
            .map_err(|_| TradingSbfError::Transition)?;
        let FundingSeedInputV5::Bytes(resolved) = resolved else {
            return Err(TradingSbfError::Transition.into());
        };
        let bytes = resolved.as_slice();
        values[ordinal][..bytes.len()].copy_from_slice(bytes);
        lengths[ordinal] = bytes.len();
        ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Transition)?;
    }
    if effect
        .resolve_funding_seed(
            action_index,
            action.seed_count() - 1,
            tail_count,
            scalars,
            identities,
        )
        .map_err(|_| TradingSbfError::Transition)?
        != FundingSeedInputV5::CanonicalBump
    {
        return Err(TradingSbfError::Transition.into());
    }
    let mut slices: [&[u8]; MAX_ACTION_SEEDS_V5 as usize] = [&[]; MAX_ACTION_SEEDS_V5 as usize];
    let mut seed = 0_usize;
    while seed < non_bump {
        slices[seed] = &values[seed][..lengths[seed]];
        seed = seed.checked_add(1).ok_or(TradingSbfError::Transition)?;
    }
    let (address, bump) = Pubkey::try_find_program_address(&slices[..non_bump], program_id)
        .ok_or(TradingSbfError::Transition)?;
    Ok(FundingSignerV5 {
        values,
        lengths,
        non_bump,
        bump: [bump],
        address,
    })
}

pub(super) fn funding_owns_coordinate_v5(effect: EffectProgramV5<'_>, coordinate: usize) -> bool {
    let mut index = 0_u16;
    while index < effect.funding_action_count() {
        let Ok(action) = effect.funding_action(index) else {
            return true;
        };
        if usize::from(action.state()) == coordinate
            || usize::from(action.counterparty()) == coordinate
            || action
                .refund_destination()
                .is_some_and(|value| usize::from(value) == coordinate)
            || action
                .system_program()
                .is_some_and(|value| usize::from(value) == coordinate)
        {
            return true;
        }
        index = match index.checked_add(1) {
            Some(index) => index,
            None => return true,
        };
    }
    false
}

fn require_funding_child_separation_v5(
    effect: EffectProgramV5<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<(), ProgramError> {
    let base = effect.base();
    let mut route = 0_u16;
    while route < base.base().route_count() {
        let count = base
            .base()
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Transition)?;
        let mut invocation = 0_u32;
        while invocation < count {
            let resolved = base
                .resolved_invocation(route, invocation, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Transition)?
                .invocation;
            let fixed_end = resolved
                .fixed_account_start
                .checked_add(resolved.fixed_account_count)
                .ok_or(TradingSbfError::Transition)?;
            for coordinate in usize::from(resolved.fixed_account_start)..usize::from(fixed_end) {
                if funding_owns_coordinate_v5(effect, coordinate) {
                    return Err(TradingSbfError::Transition.into());
                }
            }
            let mut item = 0_u32;
            while item < resolved.repeated_item_count {
                let item_start = resolved
                    .item_account_start
                    .checked_add(
                        usize::try_from(item)
                            .map_err(|_| TradingSbfError::Transition)?
                            .checked_mul(usize::from(resolved.item_account_stride))
                            .ok_or(TradingSbfError::Transition)?,
                    )
                    .ok_or(TradingSbfError::Transition)?;
                let item_end = item_start
                    .checked_add(usize::from(resolved.item_account_count))
                    .ok_or(TradingSbfError::Transition)?;
                for coordinate in item_start..item_end {
                    if funding_owns_coordinate_v5(effect, coordinate) {
                        return Err(TradingSbfError::Transition.into());
                    }
                }
                item = item.checked_add(1).ok_or(TradingSbfError::Transition)?;
            }
            invocation = invocation
                .checked_add(1)
                .ok_or(TradingSbfError::Transition)?;
        }
        route = route.checked_add(1).ok_or(TradingSbfError::Transition)?;
    }
    Ok(())
}

/// A lifecycle-selected root close is the sole physical owner of coordinate
/// zero for the whole execution. Unlike the live-root Fractional exception,
/// no child may observe or mutate the root while its terminal close is pending.
pub(super) fn require_root_lifecycle_close_child_separation_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    aliases: &[usize],
) -> Result<(), ProgramError> {
    let mut route = 0_u16;
    while route < effect.route_count() {
        let count = effect
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Transition)?;
        let mut invocation_index = 0_u32;
        while invocation_index < count {
            let invocation = effect
                .resolved_invocation(route, invocation_index, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Transition)?;
            let fixed_start = usize::from(invocation.fixed_account_start);
            let fixed_end = fixed_start
                .checked_add(usize::from(invocation.fixed_account_count))
                .ok_or(TradingSbfError::Transition)?;
            require_window_excludes_root_v3(fixed_start, fixed_end, aliases)?;
            let item_width = usize::from(invocation.item_account_count);
            let item_stride = usize::from(invocation.item_account_stride);
            let mut item = 0_u32;
            while item < invocation.repeated_item_count {
                let item_start = invocation
                    .item_account_start
                    .checked_add(
                        usize::try_from(item)
                            .map_err(|_| TradingSbfError::Transition)?
                            .checked_mul(item_stride)
                            .ok_or(TradingSbfError::Transition)?,
                    )
                    .ok_or(TradingSbfError::Transition)?;
                let item_end = item_start
                    .checked_add(item_width)
                    .ok_or(TradingSbfError::Transition)?;
                require_window_excludes_root_v3(item_start, item_end, aliases)?;
                item = item.checked_add(1).ok_or(TradingSbfError::Transition)?;
            }
            invocation_index = invocation_index
                .checked_add(1)
                .ok_or(TradingSbfError::Transition)?;
        }
        route = route.checked_add(1).ok_or(TradingSbfError::Transition)?;
    }
    Ok(())
}

pub(super) fn require_window_excludes_root_v3(
    start: usize,
    end: usize,
    aliases: &[usize],
) -> Result<(), ProgramError> {
    let mut coordinate = start;
    while coordinate < end {
        if representative_v3(coordinate, aliases)? == 0 {
            return Err(TradingSbfError::Transition.into());
        }
        coordinate = coordinate
            .checked_add(1)
            .ok_or(TradingSbfError::Transition)?;
    }
    Ok(())
}
