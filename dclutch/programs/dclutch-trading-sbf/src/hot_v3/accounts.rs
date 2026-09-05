//! Physical accounts: runtime expansion over dynamic spans, child-route
//! privilege downgrades, geometry and the borrowed-witness rules.

use super::*;

pub(super) const fn prestate_uses_variable_marker_v3(prestate: AccountPrestateV2) -> bool {
    matches!(
        prestate,
        AccountPrestateV2::AdapterAuthenticatedVariableData
    )
}

pub(super) fn projected_account_uses_variable_marker_v3(
    profile: AccountProfileV2<'_>,
    coordinate: usize,
) -> Result<bool, ProgramError> {
    let coordinate = u16::try_from(coordinate).map_err(|_| TradingSbfError::Content)?;
    let prestate = profile
        .rule(false, coordinate)
        .map_err(|_| TradingSbfError::Content)?
        .prestate();
    Ok(prestate_uses_variable_marker_v3(prestate))
}

/// Boxed, and the box is not decoration.
pub(super) struct AuthenticatedDynamicSpanWidthsV3 {
    pub(super) widths: Vec<u32>,
    /// `None` exactly when the profile declares no dynamic spans, which is the
    /// shape whose effect account width is its base width.
    pub(super) effect_span_extension: Option<EffectSpanExtensionV3>,
    pub(super) transport_span: Option<u16>,
}

/// Everything the derivation bank was still being held for once the widths had
/// been read out of it.
///
/// `ProgramV4::account_count(tail, scalars)` is
/// `base.account_count(tail) + total_extension(scalars)`, behind
/// `require_scalar_width(tail, scalars)`. The extension term reads span
/// SELECTORS, which are protected common scalars, so it does not depend on
/// `tail`; and the width guard reads the bank only through its length. Those
/// two numbers are the whole of what a later caller could still learn from the
/// bank, so carrying them is what lets the bank die in the phase that built it
/// instead of being charged against a no-op `dealloc` for the rest of the
/// instruction.
#[derive(Clone, Copy)]
pub(super) struct EffectSpanExtensionV3 {
    /// Accounts the selected EffectV4 spans add to the base account width.
    accounts: usize,
    /// Length of the bank the selection was made in, which is
    /// `EffectProgramV3::scalar_count` at the width it was derived at.
    scalar_count: usize,
}

/// Decompose `ProgramV4::account_count` into the two terms a later caller needs,
/// from a bank that is about to go out of scope.
///
/// `None` is the empty-bank shape, whose effect account width is its base
/// width and which carries no selection to restate.
fn effect_span_extension_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
) -> Result<Option<EffectSpanExtensionV3>, ProgramError> {
    if scalars.is_empty() {
        return Ok(None);
    }
    let total = effect
        .successor
        .account_count(tail_count, scalars)
        .map_err(|_| TradingSbfError::Content)?;
    let base = effect
        .base()
        .account_count(tail_count)
        .map_err(|_| TradingSbfError::Content)?;
    Ok(Some(EffectSpanExtensionV3 {
        accounts: total.checked_sub(base).ok_or(TradingSbfError::Content)?,
        scalar_count: scalars.len(),
    }))
}

/// Copy one slice into a bank on the scratch end.
///
/// Not `filled` followed by `copy_from_slice`: `filled` pushes element by
/// element, so seeding a bank that is about to be overwritten anyway would walk
/// the whole width twice.
fn scratch_bank_from_slice_v1<'region, T: Copy>(
    region: &'region HeapScratchRegionV1,
    source: &[T],
) -> Result<ScratchVecV1<'region, T>, ProgramError> {
    let mut bank = ScratchVecV1::with_capacity(region, source.len())?;
    for value in source {
        bank.push(*value)?;
    }
    Ok(bank)
}

pub(super) fn require_dynamic_span_values_v3(
    profile: AccountProfileV2<'_>,
    expected: &[u32],
    scalars: &[u64],
) -> Result<(), ProgramError> {
    if !profile.uses_dynamic_fixed_spans() {
        return if expected.is_empty() {
            Ok(())
        } else {
            Err(TradingSbfError::Content.into())
        };
    }
    let mut observed = vec![0_u32; usize::from(profile.dynamic_fixed_span_count())];
    profile
        .dynamic_span_widths_from_scalars(scalars, &mut observed)
        .map_err(|_| TradingSbfError::Content)?;
    if observed == expected {
        Ok(())
    } else {
        Err(TradingSbfError::Content.into())
    }
}

/// Derive Profile13 physical widths before account expansion without accepting
/// account-vector length as authority.
///
/// Request-owned selectors are projected once from the exact family bytes into
/// a throwaway bank. A sole non-Request selector is admitted only when the
/// authenticated strategy's canonical bank geometry requires scratch pages;
/// that page count is then derived from scalar/identity widths. Every EffectV4
/// span selector must be one of the Request-owned Profile13 selectors, while
/// the scratch transport span remains AccountProfile-only and effectless.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(super) fn authenticate_dynamic_span_widths_v3(
    profile: AccountProfileV2<'_>,
    request: RequestProfileKindV3<'_>,
    effect: SelectedEffectProgramV4<'_>,
    disposition: StrategyDispositionV2,
    tail_count: u32,
    family_request: &[u8],
    request_digest: [u8; 32],
    trusted_environment: TrustedEnvironmentObservationV3,
    scalar_count: usize,
    identity_count: usize,
) -> Result<Box<AuthenticatedDynamicSpanWidthsV3>, ProgramError> {
    if !profile.uses_dynamic_fixed_spans() {
        if profile.dynamic_fixed_span_count() != 0 || effect.successor.span_count() != 0 {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(Box::new(AuthenticatedDynamicSpanWidthsV3 {
            widths: Vec::new(),
            effect_span_extension: None,
            transport_span: None,
        }));
    }
    if profile.dynamic_fixed_span_count() == 0 {
        if effect.successor.span_count() != 0 {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(Box::new(AuthenticatedDynamicSpanWidthsV3 {
            widths: Vec::new(),
            effect_span_extension: None,
            transport_span: None,
        }));
    }
    let mut widths = vec![0_u32; usize::from(profile.dynamic_fixed_span_count())];
    // THE DERIVATION BANK COMES OFF THE SCRATCH END AND GOES BACK IN ONE STORE.
    // Six banks are built here, three of them `scalar_count` wide, and every one
    // of them is dead the moment this phase has its widths: the projection is
    // run to learn a number, not to produce a bank anything downstream reads.
    // On the upward end that made them the phase with the SECOND-LARGEST
    // per-outcome slope in the whole route -- measured on the stride-6 OpenBatch
    // ladder at HEAD, 144 of the 528 bytes each outcome cost, and 8,232 bytes
    // flat at N = 2 -- because the bump allocator's `dealloc` is a no-op and a
    // dead bank is still charged for the rest of the instruction. Off the
    // scratch end they are charged only while this phase runs.
    //
    // This is `project_hot_effects_v3`'s split, one phase earlier: the region
    // opens and closes inside this function, so `ScratchVecV1`'s borrow of it
    // is what proves no bank outlives the release, and the main execution
    // region is not open yet.
    let (transport_span, transport_page_count, effect_span_extension) = {
        let derivation = HeapScratchRegionV1::open()?;
        let mut input_scalars = ScratchVecV1::filled(&derivation, &0_u64, scalar_count)?;
        let mut input_identities = ScratchVecV1::filled(&derivation, &[0_u8; 32], identity_count)?;
        *input_identities
            .as_mut_slice()
            .get_mut(HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3)
            .ok_or(TradingSbfError::Content)? = request_digest;
        seed_trusted_environment_v3(
            trusted_environment,
            input_scalars.as_mut_slice(),
            input_identities.as_mut_slice(),
        )?;
        let mut scratch_scalars =
            scratch_bank_from_slice_v1(&derivation, input_scalars.as_slice())?;
        let mut scratch_identities =
            scratch_bank_from_slice_v1(&derivation, input_identities.as_slice())?;
        let mut projected_scalars =
            scratch_bank_from_slice_v1(&derivation, input_scalars.as_slice())?;
        let mut projected_identities =
            scratch_bank_from_slice_v1(&derivation, input_identities.as_slice())?;
        request.project_atomic(
            tail_count,
            family_request,
            ProjectionRegistersV1 {
                input_scalars: input_scalars.as_slice(),
                input_identities: input_identities.as_slice(),
                scratch_scalars: scratch_scalars.as_mut_slice(),
                scratch_identities: scratch_identities.as_mut_slice(),
                output_scalars: projected_scalars.as_mut_slice(),
                output_identities: projected_identities.as_mut_slice(),
            },
        )?;
        // `projected_scalars` is the failure-atomic output of the throwaway
        // request projection; the other banks remain phase-local validation
        // scratch.
        let transport_page_count = match classify_bank_transport_v2(
            u32::try_from(scalar_count).map_err(|_| TradingSbfError::Content)?,
            u32::try_from(identity_count).map_err(|_| TradingSbfError::Content)?,
        )
        .map_err(|_| TradingSbfError::Content)?
        {
            BankTransportV2::InlineReturnData { .. } => None,
            BankTransportV2::AuthenticatedScratchPages { page_count, .. } => Some(page_count),
        };
        let mut transport_span = None;
        let mut index = 0_u16;
        while index < profile.dynamic_fixed_span_count() {
            let span = profile
                .dynamic_fixed_span(index)
                .map_err(|_| TradingSbfError::Content)?;
            let target = ProjectionTargetV1 {
                kind: ProjectionRegisterKindV1::Scalar,
                space: ProjectionRegisterSpaceV1::Common,
                index: span.count_scalar(),
            };
            let request_owned = request.writes_register(target)?;
            let effect_owned = (0..effect.successor.span_count()).any(|effect_index| {
                effect
                    .successor
                    .span(effect_index)
                    .is_ok_and(|value| value.selector_common_scalar() == span.count_scalar())
            });
            if request_owned {
                if !effect_owned {
                    require_trailing_account_profile_only_span_v3(profile, span)?;
                }
            } else {
                if effect_owned
                    || disposition != StrategyDispositionV2::AdmittedAot
                    || transport_span.is_some()
                {
                    return Err(TradingSbfError::Content.into());
                }
                require_trailing_account_profile_only_span_v3(profile, span)?;
                let page_count = transport_page_count.ok_or(TradingSbfError::Content)?;
                *projected_scalars
                    .as_mut_slice()
                    .get_mut(usize::from(span.count_scalar()))
                    .ok_or(TradingSbfError::Content)? = u64::from(page_count);
                transport_span = Some(index);
            }
            index = index.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        let mut effect_span = 0_u16;
        while effect_span < effect.successor.span_count() {
            let selector = effect
                .successor
                .span(effect_span)
                .map_err(|_| TradingSbfError::Content)?
                .selector_common_scalar();
            if !(0..profile.dynamic_fixed_span_count()).any(|profile_index| {
                profile
                    .dynamic_fixed_span(profile_index)
                    .is_ok_and(|value| value.count_scalar() == selector)
            }) {
                return Err(TradingSbfError::Content.into());
            }
            effect_span = effect_span.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        profile
            .dynamic_span_widths_from_scalars(projected_scalars.as_slice(), &mut widths)
            .map_err(|_| TradingSbfError::Content)?;
        // The account count is not discarded and recomputed later out of a bank
        // held alive to carry it: it is decomposed into the two terms
        // `ProgramV4::account_count` is made of, one of which the caller can ask
        // the same authority for at its own tail count and the other of which
        // does not depend on one.
        let effect_span_extension =
            effect_span_extension_v3(effect, tail_count, projected_scalars.as_slice())?;
        (transport_span, transport_page_count, effect_span_extension)
    };
    if disposition == StrategyDispositionV2::AdmittedAot
        && transport_page_count.is_some() != transport_span.is_some()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(Box::new(AuthenticatedDynamicSpanWidthsV3 {
        widths,
        effect_span_extension,
        transport_span,
    }))
}

pub(super) fn authenticated_input_scratch_pages_v3<'accounts, 'info>(
    profile: AccountProfileV2<'_>,
    span_counts: &[u32],
    transport_span: Option<u16>,
    logical_accounts: &'accounts [&'accounts AccountInfo<'info>],
) -> Result<&'accounts [&'accounts AccountInfo<'info>], ProgramError> {
    let Some(transport_span) = transport_span else {
        return Ok(&[]);
    };
    if !profile.uses_dynamic_fixed_spans()
        || span_counts.len() != usize::from(profile.dynamic_fixed_span_count())
        || transport_span >= profile.dynamic_fixed_span_count()
    {
        return Err(TradingSbfError::Content.into());
    }
    let span = profile
        .dynamic_fixed_span(transport_span)
        .map_err(|_| TradingSbfError::Content)?;
    require_trailing_account_profile_only_span_v3(profile, span)?;
    let prior_width = span_counts
        .get(..usize::from(transport_span))
        .ok_or(TradingSbfError::Content)?
        .iter()
        .try_fold(0_usize, |sum, width| {
            sum.checked_add(usize::try_from(*width).map_err(|_| TradingSbfError::Content)?)
                .ok_or(TradingSbfError::Content)
        })?;
    let start = usize::from(profile.fixed_account_count())
        .checked_add(prior_width)
        .ok_or(TradingSbfError::Content)?;
    let width = usize::try_from(
        *span_counts
            .get(usize::from(transport_span))
            .ok_or(TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let end = start.checked_add(width).ok_or(TradingSbfError::Content)?;
    logical_accounts
        .get(start..end)
        .ok_or_else(|| TradingSbfError::Content.into())
}

/// Exact logical prefix the lifecycle kernel may inspect.
///
/// Dynamic fixed spans are authenticated projection transport. They are not
/// Product-N lifecycle coordinates, so the lifecycle kernel receives only the
/// stable fixed prefix after this function proves the full expanded geometry
/// and the current trailing-span layout. Projection, effects, and accelerator
/// execution continue to receive the complete expanded account bank.
pub(super) fn lifecycle_semantic_prefix_width_v3(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    expanded_width: usize,
) -> Result<usize, ProgramError> {
    let expected = if profile.uses_dynamic_fixed_spans() {
        profile
            .logical_account_count_with_dynamic_spans(tail_count, span_counts)
            .map_err(|_| TradingSbfError::Content)?
    } else {
        if !span_counts.is_empty() {
            return Err(TradingSbfError::Content.into());
        }
        profile
            .logical_account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
    };
    if expanded_width != expected {
        return Err(TradingSbfError::Content.into());
    }
    if !profile.uses_dynamic_fixed_spans() {
        return Ok(expanded_width);
    }
    let fixed = profile.fixed_account_count();
    let mut span = 0_u16;
    while span < profile.dynamic_fixed_span_count() {
        if profile
            .dynamic_fixed_span(span)
            .map_err(|_| TradingSbfError::Content)?
            .insertion_coordinate()
            != fixed
        {
            return Err(TradingSbfError::Content.into());
        }
        span = span.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(usize::from(fixed))
}

fn require_trailing_account_profile_only_span_v3(
    profile: AccountProfileV2<'_>,
    span: dclutch_account_profile_contract::v2::DynamicFixedSpanV2,
) -> Result<(), ProgramError> {
    if span.insertion_coordinate() != profile.fixed_account_count() {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

pub(super) fn expand_runtime_accounts_v3<'accounts, 'info>(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    injected: [&'accounts AccountInfo<'info>; 5],
    supplied_suffix: &'accounts [AccountInfo<'info>],
) -> Result<Vec<&'accounts AccountInfo<'info>>, ProgramError> {
    let dynamic = profile.uses_dynamic_fixed_spans();
    let logical_count = if dynamic {
        profile
            .logical_account_count_with_dynamic_spans(tail_count, span_counts)
            .map_err(|_| TradingSbfError::Content)?
    } else {
        if !span_counts.is_empty() {
            return Err(TradingSbfError::Content.into());
        }
        profile
            .logical_account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
    };
    let physical_count = if dynamic {
        profile
            .physical_account_count_with_dynamic_spans(tail_count, span_counts)
            .map_err(|_| TradingSbfError::Content)?
    } else {
        profile
            .physical_account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
    };
    if logical_count > MAX_HOT_RUNTIME_ACCOUNTS_V3
        || physical_count < injected.len()
        || supplied_suffix.len()
            != physical_count
                .checked_sub(injected.len())
                .ok_or(TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    for coordinate in 0..injected.len() {
        let representative = if dynamic {
            profile
                .representative_with_dynamic_spans(tail_count, span_counts, coordinate)
                .map_err(|_| TradingSbfError::Content)?
        } else {
            profile
                .representative(tail_count, coordinate)
                .map_err(|_| TradingSbfError::Content)?
        };
        let ordinal = if dynamic {
            profile
                .physical_account_ordinal_with_dynamic_spans(tail_count, span_counts, coordinate)
                .map_err(|_| TradingSbfError::Content)?
        } else {
            profile
                .physical_account_ordinal(tail_count, coordinate)
                .map_err(|_| TradingSbfError::Content)?
        };
        if representative != coordinate || ordinal != coordinate {
            return Err(TradingSbfError::Content.into());
        }
    }
    // Addressed in place, never concatenated into a `Vec`. See
    // [`PhysicalAccountsV4`]: the joined buffer was dead the moment the logical
    // vector existed and still cost its full physical width for the rest of the
    // instruction.
    let physical = PhysicalAccountsV4::new(&injected, supplied_suffix);
    if physical.len() != physical_count {
        return Err(TradingSbfError::Content.into());
    }
    if dynamic {
        return expand_dynamic_physical_accounts_v4(profile, tail_count, span_counts, &physical);
    }
    // One forward sweep, not one prefix recount per coordinate: see
    // `expand_dynamic_physical_accounts_v4` for why the two maps are identical.
    let packs = profile.supports_route_alias_packing();
    let mut logical = Vec::with_capacity(logical_count);
    let mut next = 0_usize;
    let mut coordinate = 0_usize;
    while coordinate < logical_count {
        let representative = profile
            .representative(tail_count, coordinate)
            .map_err(|_| TradingSbfError::Content)?;
        let resolved = if !packs {
            physical.get(coordinate).ok_or(TradingSbfError::Content)?
        } else if representative == coordinate {
            let resolved = physical.get(next).ok_or(TradingSbfError::Content)?;
            next = next.checked_add(1).ok_or(TradingSbfError::Content)?;
            resolved
        } else {
            if representative >= coordinate
                || profile
                    .representative(tail_count, representative)
                    .map_err(|_| TradingSbfError::Content)?
                    != representative
            {
                return Err(TradingSbfError::Content.into());
            }
            *logical
                .get(representative)
                .ok_or(TradingSbfError::Content)?
        };
        logical.push(resolved);
        coordinate = coordinate.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(logical)
}

/// The child-CPI view of the logical account vector, materialised one window at
/// a time instead of all at once.
///
/// A window is still a `&[AccountInfo]` to the composition that receives it;
/// only the 91-wide intermediate is gone.
///
/// # What did NOT become lazy
///
/// The whole-frame privilege check did not. `new` walks every coordinate and
/// applies the same refusal the eager build did, materialising nothing: a
/// declaration that is writable where the transaction is not, or whose
/// executability disagrees with the account, refuses the instruction before any
/// child runs -- not when some later route happens to gather that coordinate.
/// Deferring that check would have quietly narrowed the refusal set to the
/// coordinates a particular request touches, which is a different program.
#[derive(Clone, Copy)]
pub struct DowngradedEffectAccountsV3<'a, 'accounts, 'info> {
    logical: &'a [&'accounts AccountInfo<'info>],
    declared: &'a [u8],
}

impl<'a, 'accounts, 'info> DowngradedEffectAccountsV3<'a, 'accounts, 'info> {
    /// Logical width, which is the width the eager vector had.
    pub const fn len(self) -> usize {
        self.logical.len()
    }

    /// A frame with no coordinates at all, which no admitted profile produces.
    pub const fn is_empty(self) -> bool {
        self.logical.is_empty()
    }

    /// One coordinate's child view, from the privilege byte decoded once.
    pub fn view(self, coordinate: usize) -> Result<AccountInfo<'info>, ProgramError> {
        let declared = *self
            .declared
            .get(coordinate)
            .ok_or(TradingSbfError::Content)?;
        let mut logical = (*self
            .logical
            .get(coordinate)
            .ok_or(TradingSbfError::Content)?)
        .clone();
        logical.is_signer = declared & DECLARED_SIGNER_V3 != 0;
        logical.is_writable = declared & DECLARED_WRITABLE_V3 != 0;
        Ok(logical)
    }

    /// How many coordinates in one window carry a given program.
    ///
    /// A key comparison only, so it reads the logical vector directly and
    /// materialises nothing at all -- the privilege downgrade cannot change an
    /// account's address.
    pub fn count_program_in_window(
        self,
        start: usize,
        count: usize,
        program: &Pubkey,
    ) -> Result<usize, ProgramError> {
        let end = start.checked_add(count).ok_or(TradingSbfError::Content)?;
        Ok(self
            .logical
            .get(start..end)
            .ok_or(TradingSbfError::Content)?
            .iter()
            .filter(|account| account.key == program)
            .count())
    }

    /// One child account vector, allocated ONCE at the exact width this
    /// invocation will fill plus the child program every composition pushes on
    /// the end.
    ///
    /// Every composition started this buffer empty and grew it through
    /// `extend_window`. On the SBF bump allocator, which never frees, that is
    /// the whole doubling ladder AND a final reallocation for the push, and
    /// every buffer it walked through stays charged for the rest of the
    /// instruction. Measured on the canonical Direct bundle at the moment the
    /// heap is scarcest -- inside the child-route walk, past the projection --
    /// **7,195 bytes for one Claims invocation and 5,691 for one Custody
    /// invocation**, against a live width of 48 bytes per account.
    ///
    /// The width is a fact about the resolved invocation, known before the
    /// first window is appended: the fixed frame, plus one item subframe per
    /// repeated item, plus the program. `extend_window`'s own `try_reserve` is
    /// then satisfied and never grows.
    ///
    /// It is bounded by the logical frame because a hostile geometry must
    /// refuse rather than ask the allocator for the refusal.
    ///
    /// It takes the buffer by `&mut` rather than returning a fresh one so the
    /// walk can hand the SAME buffer to every invocation: on this allocator a
    /// per-invocation frame is a per-invocation charge, and an `Each` route
    /// over N items charged N of them. Clearing keeps the capacity, so the
    /// reservation is satisfied by whatever the widest invocation so far
    /// already bought.
    pub fn reserve_invocation_frame(
        self,
        output: &mut Vec<AccountInfo<'info>>,
        invocation: dclutch_effect_kernel::v3::ResolvedInvocationV3,
    ) -> Result<(), ProgramError> {
        let items = usize::try_from(invocation.repeated_item_count)
            .map_err(|_| TradingSbfError::Content)?
            .checked_mul(usize::from(invocation.item_account_count))
            .ok_or(TradingSbfError::Content)?;
        let exact = usize::from(invocation.fixed_account_count)
            .checked_add(items)
            .and_then(|total| total.checked_add(1))
            .ok_or(TradingSbfError::Content)?;
        if exact
            > self
                .logical
                .len()
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?
        {
            return Err(TradingSbfError::Content.into());
        }
        output.clear();
        output
            .try_reserve_exact(exact)
            .map_err(|_| TradingSbfError::HeapExhausted)?;
        Ok(())
    }

    /// Append one contiguous window's child views to a caller-owned buffer.
    ///
    /// This is the only shape any consumer ever needed: each composition's
    /// `invocation_accounts` was `output.extend_from_slice(accounts[a..b])` over
    /// windows it computes from its own resolved invocation.
    pub fn extend_window(
        self,
        output: &mut Vec<AccountInfo<'info>>,
        start: usize,
        count: usize,
    ) -> Result<(), ProgramError> {
        let end = start.checked_add(count).ok_or(TradingSbfError::Content)?;
        if end > self.logical.len() {
            return Err(TradingSbfError::Content.into());
        }
        output
            .try_reserve(count)
            .map_err(|_| TradingSbfError::HeapExhausted)?;
        let mut coordinate = start;
        while coordinate < end {
            output.push(self.view(coordinate)?);
            coordinate = coordinate.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        Ok(())
    }
}

/// ONE set of child-CPI buffers, owned by a child walk and reused by every
/// invocation on that walk.
///
/// # The measurement this exists for
///
/// So the walk owns one set, sized by whatever the widest invocation so far
/// needed, and every composition fills it instead of allocating. `clear()`
/// keeps capacity, so the second invocation's reservations are already
/// satisfied and charge nothing at all.
///
/// # The one buffer that is NOT reused, and why
///
/// [`Self::returned`] is refilled from the return-data syscall each time,
/// because its bytes do not die with the invocation: they are MOVED into the
/// receipt bank, where a later route resolves its declared dependency against
/// them. Reusing that allocation would hand the bank a buffer the next
/// invocation overwrites. What this type does own about it is that the syscall
/// is read EXACTLY ONCE per child -- the composition verifies the receipt and
/// the walk banks it out of the same vector, where before each read the syscall
/// again into a second vector of its own.
pub struct ChildInvocationBuffersV3<'info> {
    /// The resolved child account frame, with the callee appended last.
    pub accounts: Vec<AccountInfo<'info>>,
    /// The child instruction's account list, in frame order.
    pub metas: Vec<AccountMeta>,
    /// The child instruction's wire.
    pub data: Vec<u8>,
    /// Producer the return-data syscall named for the last invocation.
    pub producer: Pubkey,
    /// Bytes the last invocation returned, read from the syscall exactly once.
    pub returned: Vec<u8>,
}

impl<'info> ChildInvocationBuffersV3<'info> {
    /// An empty set. Every buffer buys its capacity at its first invocation.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            accounts: Vec::new(),
            metas: Vec::new(),
            data: Vec::new(),
            // A nonzero internal sentinel means the default capture allocates
            // exactly as the SDK helper does. The walk replaces it with zero
            // only for its last child, whose dead wire may be reused.
            producer: Pubkey::new_from_array([1; 32]),
            returned: Vec::new(),
        }
    }

    /// Buy the exact widest authenticated child wire once, before any child
    /// invocation can make a smaller permanent allocation on the SBF bump
    /// heap.
    pub fn reserve_wire_exact(&mut self, capacity: usize) -> Result<(), ProgramError> {
        if !self.data.is_empty() {
            return Err(TradingSbfError::Content.into());
        }
        self.data
            .try_reserve_exact(capacity)
            .map_err(|_| TradingSbfError::HeapExhausted.into())
    }

    /// Replace the child wire with `request`, reusing the buffer's capacity.
    pub fn set_wire(&mut self, request: &[u8]) -> Result<(), ProgramError> {
        self.data.clear();
        self.data
            .try_reserve(request.len())
            .map_err(|_| TradingSbfError::HeapExhausted)?;
        self.data.extend_from_slice(request);
        Ok(())
    }

    /// Derive the child's account list from the frame gathered so far.
    ///
    /// The privilege rule is the one every composition on this walk applied
    /// for itself: the account's own declared writability, and signer for the
    /// release-pinned caller authority at coordinate 0 or wherever the frame
    /// already declares one. Call this BEFORE the callee is appended -- the
    /// callee is not a member of the child's account list.
    #[inline(never)]
    pub fn fill_metas(&mut self) -> Result<(), ProgramError> {
        self.metas.clear();
        self.metas
            .try_reserve(self.accounts.len())
            .map_err(|_| TradingSbfError::HeapExhausted)?;
        for (index, account) in self.accounts.iter().enumerate() {
            let signer = index == 0 || account.is_signer;
            self.metas.push(if account.is_writable {
                AccountMeta::new(*account.key, signer)
            } else {
                AccountMeta::new_readonly(*account.key, signer)
            });
        }
        Ok(())
    }

    /// Append the callee to the account frame the CPI passes to the runtime.
    pub fn push_callee(&mut self, callee: &AccountInfo<'info>) -> Result<(), ProgramError> {
        self.accounts
            .try_reserve(1)
            .map_err(|_| TradingSbfError::HeapExhausted)?;
        self.accounts.push(callee.clone());
        Ok(())
    }

    /// Invoke the callee with the buffers this set holds.
    ///
    /// The buffers are handed to the membrane by `&mut` and come back with
    /// their allocations intact; see
    /// [`crate::entrypoint_adapter::invoke_signed_owned_v1`] for what that
    /// saves and what it reproduces.
    #[inline(never)]
    pub fn invoke(
        &mut self,
        callee: &Pubkey,
        signers_seeds: &[&[&[u8]]],
    ) -> Result<(), ProgramError> {
        let Self {
            accounts,
            metas,
            data,
            ..
        } = self;
        crate::entrypoint_adapter::invoke_signed_owned_v1(
            callee,
            metas,
            data,
            accounts,
            signers_seeds,
        )
    }

    /// Read the return-data syscall ONCE for the invocation just made.
    #[inline(never)]
    pub fn capture_return(&mut self) -> Result<(), ProgramError> {
        if self.producer == Pubkey::default() {
            if !self.returned.is_empty() {
                return Err(TradingSbfError::Content.into());
            }
            let producer = crate::entrypoint_adapter::get_return_data_into_v1(&mut self.data)?
                .ok_or(TradingSbfError::Transition)?;
            core::mem::swap(&mut self.data, &mut self.returned);
            self.producer = producer;
            return Ok(());
        }
        let (producer, returned) = get_return_data().ok_or(TradingSbfError::Transition)?;
        self.producer = producer;
        self.returned = returned;
        Ok(())
    }

    /// Move the captured return data out, for the receipt bank to own.
    #[must_use]
    pub fn take_returned(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.returned)
    }
}

impl Default for ChildInvocationBuffersV3<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[inline(never)]
pub(crate) fn downgraded_effect_accounts_v3<'a, 'accounts, 'info>(
    logical_accounts: &'a [&'accounts AccountInfo<'info>],
    declared: &'a [u8],
) -> Result<DowngradedEffectAccountsV3<'a, 'accounts, 'info>, ProgramError> {
    if logical_accounts.len() != declared.len() {
        return Err(TradingSbfError::Content.into());
    }
    Ok(DowngradedEffectAccountsV3 {
        logical: logical_accounts,
        declared,
    })
}

/// Declared-signer bit of a packed privilege byte.
const DECLARED_SIGNER_V3: u8 = 1;
/// Declared-writable bit of a packed privilege byte.
const DECLARED_WRITABLE_V3: u8 = 2;

/// Decode every logical coordinate's declared route privileges ONCE, and check
/// the whole frame while doing it.
///
/// The check is the whole-frame one the materialised bank performed, unchanged
/// and still eager: a declaration writable where the transaction is not, or
/// whose executability disagrees with the account, refuses the instruction here
/// -- not when some later route happens to gather that coordinate.
pub(crate) fn child_route_privileges_v3(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    logical_accounts: &[&AccountInfo<'_>],
) -> Result<Vec<u8>, ProgramError> {
    let dynamic = profile.uses_dynamic_fixed_spans();
    let logical_count = if dynamic {
        dynamic_logical_account_count_v4(profile, tail_count, span_counts)?
    } else {
        if !span_counts.is_empty() {
            return Err(TradingSbfError::Content.into());
        }
        profile
            .logical_account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
    };
    if logical_accounts.len() != logical_count {
        return Err(TradingSbfError::Content.into());
    }
    let mut declared = Vec::new();
    declared
        .try_reserve_exact(logical_count)
        .map_err(|_| TradingSbfError::HeapExhausted)?;
    let mut coordinate = 0_usize;
    while coordinate < logical_count {
        let privileges = if dynamic {
            dynamic_declared_privileges_v4(profile, tail_count, span_counts, coordinate)?
        } else {
            profile
                .route_privileges(
                    tail_count,
                    profile
                        .representative(tail_count, coordinate)
                        .map_err(|_| TradingSbfError::Content)?,
                )
                .map_err(|_| TradingSbfError::Content)?
        };
        require_child_route_privileges_v3(
            logical_accounts
                .get(coordinate)
                .ok_or(TradingSbfError::Content)?,
            privileges,
        )?;
        declared.push(
            u8::from(privileges.signer())
                .wrapping_mul(DECLARED_SIGNER_V3)
                .wrapping_add(u8::from(privileges.writable()).wrapping_mul(DECLARED_WRITABLE_V3)),
        );
        coordinate = coordinate.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(declared)
}

/// Refuse a declared privilege the physical account cannot carry.
///
/// An authenticated route alias declares no privileges of its own -- the
/// AccountProfile validator's route-alias contract requires the producer to
/// emit it privilege-free -- so the representative coordinate is the sole owner
/// of every privilege fact about the physical account, signer and writable
/// included, not only executability. Reading them from the alias produced a
/// readonly non-signer meta for an account the authenticated FrameSpec-derived
/// representative rule states as writable, which the child program cannot
/// honour; nothing about the alias ever expressed a per-route downgrade,
/// because there is no privilege field in an alias to express one with.
///
/// The other direction is refused here for writability: a declaration never
/// becomes a writable meta for an account the transaction did not include as
/// writable, because no CPI can escalate that and the runtime's own refusal
/// names nothing useful. Signer is deliberately not required of the
/// transaction: a child route's caller authority is a Trading PDA that signs
/// only inside the child CPI, through `invoke_signed`, which is exactly the
/// privilege the FrameSpec owns and the outer frame never grants; a meta that
/// claims a signer Trading cannot produce seeds for still fails closed in the
/// runtime. Executability is exact in both directions: it is a property of the
/// account, never granted or suppressed by a route.
fn require_child_route_privileges_v3(
    account: &AccountInfo<'_>,
    declared: RouteAccountPrivilegesV2,
) -> Result<(), ProgramError> {
    if (declared.writable() && !account.is_writable) || declared.executable() != account.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct CommonProjectionBindingsV3 {
    pub(super) selected_config: [u8; 32],
    pub(super) selected_product_record: [u8; 32],
    pub(super) authenticated_product_record: [u8; 32],
    pub(super) market_product: [u8; 32],
    pub(super) runtime_product: [u8; 32],
    pub(super) product_semantic_basis: [u8; 32],
    pub(super) authenticated_semantic_basis: [u8; 32],
    pub(super) authenticated_linked_basis: [u8; 32],
}

pub(super) fn require_common_projection_bindings_v3(
    bindings: CommonProjectionBindingsV3,
) -> Result<(), ProgramError> {
    if bindings.selected_config == [0; 32]
        || bindings.selected_product_record == [0; 32]
        || bindings.selected_product_record != bindings.authenticated_product_record
        || bindings.market_product == [0; 32]
        || bindings.market_product != bindings.runtime_product
        || bindings.product_semantic_basis == [0; 32]
        || bindings.product_semantic_basis != bindings.authenticated_semantic_basis
        || bindings.authenticated_linked_basis == [0; 32]
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

/// The product's outcome count and the profile's projected tail must agree --
/// WHEN THE PROFILE PROJECTS ONE.
///
/// Nothing is weakened. A profile that DOES project a tail is held to the same
/// equality it always was, and a profile that does not is held to the only
/// honest value there is -- it may not arrive carrying a width it never
/// declared. The `< 2` floor is a fact about the MARKET and applies either way.
pub(super) fn require_tail_count_agreement_v3(
    product_outcome_count: u32,
    projected_tail_count: Option<u32>,
) -> Result<(), ProgramError> {
    if product_outcome_count < 2 {
        return Err(TradingSbfError::Content.into());
    }
    match projected_tail_count {
        Some(projected) if projected != product_outcome_count => {
            Err(TradingSbfError::Content.into())
        }
        Some(_) | None => Ok(()),
    }
}

pub(super) fn require_common_projection_permissions_v3(
    permissions: &[AccountPermission],
) -> Result<(), ProgramError> {
    if permissions.get(1) != Some(&AccountPermission::read_only())
        || permissions.get(2) != Some(&AccountPermission::read_only())
        || permissions.get(3) != Some(&AccountPermission::read_only())
        || permissions.get(4) != Some(&AccountPermission::read_only())
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

pub(super) fn lifecycle_request_target_v4(target: LifecycleRegisterTargetV3) -> ProjectionTargetV1 {
    ProjectionTargetV1 {
        kind: match target.kind() {
            LifecycleRegisterKindV3::Scalar => ProjectionRegisterKindV1::Scalar,
            LifecycleRegisterKindV3::Identity => ProjectionRegisterKindV1::Identity,
        },
        space: match target.scope() {
            CoordinateScopeV3::Fixed => ProjectionRegisterSpaceV1::Common,
            CoordinateScopeV3::Item => ProjectionRegisterSpaceV1::Item,
        },
        index: target.index(),
    }
}

pub(super) fn lifecycle_transition_target_v4(target: LifecycleRegisterTargetV3) -> RegisterWriteTargetV3 {
    RegisterWriteTargetV3 {
        kind: match target.kind() {
            LifecycleRegisterKindV3::Scalar => RegisterKindV3::Scalar,
            LifecycleRegisterKindV3::Identity => RegisterKindV3::Identity,
        },
        space: match target.scope() {
            CoordinateScopeV3::Fixed => RegisterSpaceV3::Common,
            CoordinateScopeV3::Item => RegisterSpaceV3::Item,
        },
        index: target.index(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn require_geometry(
    account: AccountProfileV2<'_>,
    request: RequestProfileKindV3<'_>,
    transition: TransitionProgramV3<'_>,
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    family_request: &[u8],
    runtime_accounts: usize,
    span_counts: &[u32],
    effect_span_extension: Option<&EffectSpanExtensionV3>,
) -> Result<(), ProgramError> {
    request.require_request_shape(tail_count, family_request)?;
    let request_v1 = request.v1();
    let expected_accounts = if account.uses_dynamic_fixed_spans() {
        account
            .logical_account_count_with_dynamic_spans(tail_count, span_counts)
            .map_err(|_| TradingSbfError::Content)?
    } else {
        if !span_counts.is_empty() {
            return Err(TradingSbfError::Content.into());
        }
        account
            .logical_account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
    };
    let base_effect_accounts = effect
        .base()
        .account_count(tail_count)
        .map_err(|_| TradingSbfError::Content)?;
    let effect_accounts = match effect_span_extension {
        None => base_effect_accounts,
        Some(extension) => {
            // `ProgramV4::require_scalar_width`, restated against the width the
            // span selection was made at instead of the bank it was made in.
            // That guard reads the bank only through its length, and this is
            // the same comparison with the same two authorities -- the effect's
            // own `scalar_count`, at this function's tail count and at the one
            // the derivation ran at. Dropping it would have been a lost
            // refusal: nothing else on this path compares the two widths for a
            // route that carves no caller authorities.
            if effect
                .base()
                .scalar_count(tail_count)
                .map_err(|_| TradingSbfError::Content)?
                != extension.scalar_count
            {
                return Err(TradingSbfError::Content.into());
            }
            base_effect_accounts
                .checked_add(extension.accounts)
                .ok_or(TradingSbfError::Content)?
        }
    };
    // The account and effect item strides are the SAME NUMBER only when the
    // profile has no dynamic spans, and requiring it unconditionally made every
    // Profile13 family unrunnable.
    //
    // `AccountProfileV2` forces the two cases apart itself (`v2.rs:1103` and
    // `:1110`): with no dynamic spans `item_account_stride` MUST be zero, and
    // with them it MUST be nonzero. Under spans the field is not a semantic
    // per-item account count at all -- `dynamic_account_width` never multiplies
    // by it, computing `fixed + sum(span_counts)` and ignoring `tail_count`
    // entirely. The General adapter says the same thing in its own words at
    // `general-adapter-contract/src/artifacts_v3.rs:619`: "the item-rule table is
    // repurposed exclusively as the dynamic fixed-span template bank ... physical
    // scratch-page geometry, not a Product-N semantic account stride."
    //
    // So General's own bundle validator REQUIRES the two to differ -- account
    // stride `GENERAL_SCRATCH_PAGE_RULE_STRIDE_V3` = 1 at `:620`, effect stride
    // 0 at `:634` -- while this function required them equal. Both could not
    // hold, and the runtime side is the one that was wrong.
    //
    // Nothing is loosened: the effect is still pinned, to 0, which is what an
    // effect with no per-item accounts declares. In the no-span branch the
    // account stride is already 0, so the equality below is exactly today's
    // behaviour.
    // Asked, not restated. `AccountProfileV2` owns both sides of this and is the
    // single author; open-coding it here is what refused every Profile13 family,
    // and a bundle-builder test that hand-copied the same predicate went red
    // against a law this function no longer had.
    let item_account_stride_agrees =
        account.admits_effect_item_account_stride(effect.item_account_stride());
    if expected_accounts != runtime_accounts
        || !item_account_stride_agrees
        || effect_accounts > expected_accounts
        || account.fixed_account_count() != effect.fixed_account_count()
        || account.common_scalar_count() != request_v1.common_scalar_count()
        || account.item_scalar_stride() != request_v1.item_scalar_stride()
        || account.common_identity_count() != request_v1.common_identity_count()
        || account.item_identity_stride() != request_v1.item_identity_stride()
        || account.common_scalar_count() != transition.common_scalar_count()
        || account.item_scalar_stride() != transition.item_scalar_stride()
        || account.common_identity_count() != transition.common_identity_count()
        || account.item_identity_stride() != transition.item_identity_stride()
        || account.common_scalar_count() != effect.common_scalar_count()
        || account.item_scalar_stride() != effect.item_scalar_stride()
        || account.common_identity_count() != effect.common_identity_count()
        || account.item_identity_stride() != effect.item_identity_stride()
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

#[inline(never)]
pub(super) fn require_borrowed_witness_coverage_v3<'a>(
    request_profile: RequestProfileKindV3<'a>,
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    family_request: &'a [u8],
) -> Result<(), ProgramError> {
    if effect.successor.range_count() != 0 {
        effect
            .successor
            .validate_request_coverage(family_request.len(), tail_count, scalars, identities)
            .map_err(|_| ProgramError::from(TradingSbfError::SuccessorCoverage))?;
    }
    let RequestProfileKindV3::Borrowed(profile) = request_profile else {
        return Ok(());
    };
    let (_, declared_witness) = profile
        .split_request(tail_count, family_request)
        .map_err(|_| TradingSbfError::BorrowedWitnessBytes)?;
    let policy = profile.witness_policy();
    let expected_role = borrowed_witness_role_v3(policy.consumer_role);
    let mut borrower_count = 0_u16;
    let mut route_index = 0_u16;
    while route_index < effect.route_count() {
        // The effect artifact's own table failing to decode stays `Content`:
        // that IS bytes being wrong, and it is not this function's accusation.
        // Everything below it is, and each half now says which.
        let route = effect
            .route(route_index)
            .map_err(|_| TradingSbfError::Content)?;
        let ranges = BorrowedRouteRangesV4::new(
            effect.successor,
            route_index,
            tail_count,
            scalars,
            family_request,
        );
        let range_count = ranges.count()?;
        // ONE SPELLING, and it is the successor's. A borrowed witness is a
        // range in the Effect V4 range table bound to a route; the V3 route
        // bit is not an alternative this rule admits, because the KERNEL does
        // not admit it either -- `EffectProgramV4::validate_range_table`
        // refuses a V4 program any of whose base routes carries the bit, for
        // every artifact, whatever its range count. Requiring the bit under V4,
        // which is what stood here, was requiring a shape the kernel refuses to
        // represent.
        //
        // That law is restated rather than assumed, because the shipped Hot
        // path does not re-run it: `from_sealed` takes `decode_shape` alone
        // (decision 0005), so a sealed artifact carrying the bit reaches this
        // walk with the table sweep never having run over it.
        if route.borrows_witness() {
            log_borrowed_witness_refusal_v3(
                4,
                u64::from(route_index),
                u64::from(range_count),
                0,
                0,
            );
            return Err(TradingSbfError::BorrowedWitnessRoute.into());
        }
        if range_count == 0 {
            route_index = route_index.checked_add(1).ok_or(TradingSbfError::Width)?;
            continue;
        }
        borrower_count = borrower_count
            .checked_add(1)
            .ok_or(TradingSbfError::Width)?;
        let invocations = effect
            .invocation_count(route_index, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?;
        let shape = u64::from(route.role() != expected_role)
            | (u64::from(route.kind() != dclutch_effect_kernel::v3::RouteKindV3::Once) << 1)
            | (u64::from(route.fixed_request_bytes() != 0) << 2)
            | (u64::from(route.item_request_bytes() != 0) << 3)
            | (u64::from(invocations != 1) << 4)
            | (u64::from(range_count > 1) << 5);
        if shape != 0 {
            log_borrowed_witness_refusal_v3(
                1,
                u64::from(route_index),
                shape,
                u64::from(invocations),
                u64::from(range_count),
            );
            return Err(TradingSbfError::BorrowedWitnessRoute.into());
        }
        let invocation = effect
            .resolved_invocation(route_index, 0, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?;
        // The bit was refused above, so the resolution cannot carry a witness;
        // this is the belt on that. `resolve_borrowed_witness` reads the bit
        // and nothing else, and the two readings must agree.
        if invocation.borrowed_witness.is_some() {
            log_borrowed_witness_refusal_v3(2, u64::from(route_index), 0, 0, 0);
            return Err(TradingSbfError::BorrowedWitnessRoute.into());
        }
        if invocation.request_len != 0 || ranges.range(0)? != declared_witness {
            return Err(TradingSbfError::BorrowedWitnessBytes.into());
        }
        route_index = route_index.checked_add(1).ok_or(TradingSbfError::Width)?;
    }
    if borrower_count != 1 {
        log_borrowed_witness_refusal_v3(
            3,
            u64::from(borrower_count),
            u64::from(effect.route_count()),
            u64::from(effect.successor.range_count()),
            u64::from(tail_count),
        );
        Err(TradingSbfError::BorrowedWitnessRoute.into())
    } else {
        Ok(())
    }
}

/// Print which witness-coverage conjunct refused, on the refusing path only.
///
/// [`TradingSbfError::BorrowedWitnessRoute`] is one accusation over four
/// sites and six conjuncts, and a u32 cannot carry which. A validator log is
/// where a reader looks first, so the fact the program already computed is
/// written there rather than left to be bisected -- the recovery this tree
/// pays for by hand every time a `map_err` discards a cause.
///
/// The five words are `site`, then the site's own operands:
///
/// * `site 1` -- the borrower's SHAPE: `route_index`, a conjunct mask
///   (bit 0 role, bit 1 kind, bit 2 fixed request bytes, bit 3 item request
///   bytes, bit 4 invocation count, bit 5 more than one borrowed range), the
///   invocation count, and the route's borrowed-range count.
/// * `site 2` -- the borrower's resolution carried a V3 borrowed witness,
///   which the route-bit refusal above says it cannot: `route_index`.
/// * `site 3` -- the effect's borrower COUNT is not one: the count, the
///   effect's route count, its successor range count, and the tail count.
/// * `site 4` -- a base route carries the retired V3 `borrows_witness` bit,
///   which no Effect V4 may: `route_index` and its borrowed-range count.
#[cold]
#[inline(never)]
fn log_borrowed_witness_refusal_v3(site: u64, first: u64, second: u64, third: u64, fourth: u64) {
    solana_program::log::sol_log("dclutch-hot:borrowed-witness");
    solana_program::log::sol_log_64(site, first, second, third, fourth);
}

const fn borrowed_witness_role_v3(role: BorrowedWitnessRoleV3) -> FixedRole {
    match role {
        BorrowedWitnessRoleV3::Core => FixedRole::Core,
        BorrowedWitnessRoleV3::Claims => FixedRole::Claims,
        BorrowedWitnessRoleV3::Resolution => FixedRole::Resolution,
        BorrowedWitnessRoleV3::Custody => FixedRole::Custody,
    }
}

/// Hold the borrowed witness's consumer to the profile's declared receipt.
///
/// Reached for every child, and it must recognise the borrower the same way
/// [`require_borrowed_witness_coverage_v3`] does -- by its Effect V4 range.
/// Keyed on the V3 `borrowed_witness` alone, as it was, this returns `Ok` for
/// a V4 successor's consumer, which is not a passed check but an unasked
/// question and the exact shape of a lost refusal: the declared
/// `child_receipt_magic` and `child_receipt_bytes` would bind nothing on the
/// one route the policy exists to bind.
pub(super) fn require_borrowed_witness_receipt_v3(
    request_profile: RequestProfileKindV3<'_>,
    borrowed_range_count: u16,
    role: FixedRole,
    receipt: &[u8],
) -> Result<(), ProgramError> {
    let RequestProfileKindV3::Borrowed(profile) = request_profile else {
        return Ok(());
    };
    if borrowed_range_count == 0 {
        return Ok(());
    }
    let policy: BorrowedWitnessPolicyV3 = profile.witness_policy();
    if role != borrowed_witness_role_v3(policy.consumer_role)
        || receipt.len()
            != usize::try_from(policy.child_receipt_bytes).map_err(|_| TradingSbfError::Content)?
        || receipt.get(..8) != Some(policy.child_receipt_magic.as_slice())
    {
        Err(TradingSbfError::Transition.into())
    } else {
        Ok(())
    }
}

/// Resolve every runtime coordinate to its canonical representative once.
///
/// Exact capacity: a fallible `collect` reports a zero lower bound, which walks
/// the never-freeing SBF bump allocator through its whole doubling ladder.
pub(super) fn representative_coordinates_v3(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    runtime_account_count: usize,
) -> Result<Vec<usize>, ProgramError> {
    let dynamic = profile.uses_dynamic_fixed_spans();
    let mut output = Vec::with_capacity(runtime_account_count);
    for coordinate in 0..runtime_account_count {
        let representative = if dynamic {
            profile.representative_with_dynamic_spans(tail_count, span_counts, coordinate)
        } else {
            profile.representative(tail_count, coordinate)
        }
        .map_err(|_| TradingSbfError::Content)?;
        output.push(representative);
    }
    Ok(output)
}

/// Resolve the authenticated runtime tail width for one selected profile.
///
/// A profile that declares a tail-count projection binds its own tail scalar
/// to the independently authenticated Product Runtime V3 outcome count. That
/// binding is *checked*, not assumed: the full account projection runs at this
/// width in `project_account_and_request_registers_v3`, and
/// `require_projected_tail_count_agreement_v3` refuses unless the profile's own
/// projected tail scalar equals the same authenticated count.
///
/// Discovering the width by running the account projection at a fictitious
/// `tail_count` of zero cannot work and was never load-bearing. It cannot work
/// because a fixed rule with a nonzero `data_item_stride` — Profile 14's
/// Portfolio, linked-basis and Claims records among them — has no valid width
/// at tail zero, so `validate_accounts` refuses with `DataLengthMismatch`
/// before the projection reads anything. It was never load-bearing because the
/// only consumer of the discovered value immediately required it to equal the
/// authenticated Product outcome count anyway.
/// The profile's tail count, and whether it has one AT ALL.
///
/// `None` is not "zero". It says the AccountProfile declares no
/// `OP_PROJECT_TAIL_COUNT_U32`, which is a legitimate and common shape -- a
/// fixed topology with no item accounts has no width to project and no business
/// carrying one. Collapsing that to `0` is what made
/// `require_tail_count_agreement_v3` unsatisfiable for every such profile; see
/// the note there.
pub(super) fn project_tail_count(
    profile: AccountProfileV2<'_>,
    authenticated_product_tail_count: u32,
) -> Result<Option<u32>, ProgramError> {
    if profile
        .tail_count_projection()
        .map_err(|_| TradingSbfError::Content)?
        .is_none()
    {
        return Ok(None);
    }
    Ok(Some(authenticated_product_tail_count))
}

pub(super) fn require_projected_tail_count_agreement_v3(
    profile: AccountProfileV2<'_>,
    authenticated_product_tail_count: u32,
    scalars: &[u64],
) -> Result<(), ProgramError> {
    let Some(projection) = profile
        .tail_count_projection()
        .map_err(|_| TradingSbfError::Content)?
    else {
        return Ok(());
    };
    if scalars.get(usize::from(projection.register()))
        != Some(&u64::from(authenticated_product_tail_count))
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}
