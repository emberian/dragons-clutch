//! Child routes: the walk, its preflight and execution, receipts, the
//! disjointness rules between local mutation and child reach, and role carriers.

use super::*;

#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn decode_claims_composition_boxed_v3<'request>(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    request_bank: &'request [u8],
    family_request: &'request [u8],
    parent: ClaimsCompositionParentV3,
) -> Result<HeapBoxV3<ClaimsCompositionV3<'request>>, ProgramError> {
    let external = if family_request.get(..8)
        == Some(dclutch_claims::fractional::FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2.as_slice())
    {
        let request =
            dclutch_claims::fractional::FractionalExposureRequestV2::decode(family_request)
                .map_err(|_| TradingSbfError::Content)?;
        let fixed_account_count = match request.action() {
            dclutch_claims::fractional::FractionalExposureActionV2::Wrap
            | dclutch_claims::fractional::FractionalExposureActionV2::WholeUnwrap => {
                dclutch_claims::fractional::FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3
            }
            dclutch_claims::fractional::FractionalExposureActionV2::TerminalRedeem
            | dclutch_claims::fractional::FractionalExposureActionV2::TerminalZeroBurn => {
                dclutch_claims::fractional::FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3
            }
            _ => return Err(TradingSbfError::Content.into()),
        };
        if request.input().release_set != parent.release_set
            || request.input().market != parent.market
            || dclutch_sha256_adapter::digest(family_request) != parent.parent_request_digest
        {
            return Err(TradingSbfError::Content.into());
        }
        Some(
            ClaimsExternalOnceV3::new(
                family_request,
                u16::try_from(fixed_account_count).map_err(|_| TradingSbfError::Content)?,
            )
            .map_err(|_| TradingSbfError::Content)?,
        )
    } else if family_request.get(..8)
        == Some(
            dclutch_claims::fractional_claim_check_v1::FRACTIONAL_CLAIM_CHECK_COMPACT_MAGIC_V1
                .as_slice(),
        )
    {
        // The second external once-route this family has ever had, and the
        // first that is not an exposure action. It is here rather than beside
        // the exposure arm because its wire is a different type at a different
        // width: a fractional compaction carries a TerminalSettlementRequestV3
        // verbatim, so `FractionalExposureRequestV2::decode` would refuse it,
        // and a shared arm would have to decode by shape rather than by magic.
        let request =
            dclutch_claims::fractional_claim_check_compaction_request_v1::FractionalCompactToClaimCheckRequestV1::decode(
                family_request,
            )
            .map_err(|_| TradingSbfError::Content)?;
        // The frame width has ONE author, and it is not this function. The
        // exposure arm above selects between two widths by action; compaction
        // has exactly one, declared beside the roles that occupy it, so a
        // further account changes the constant and this arm follows.
        let fixed_account_count =
            dclutch_claims::fractional_claim_check_v1::FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1;
        // The same three bindings the exposure arm makes, and they are what
        // make admitting an external route safe at all: the caller has already
        // authenticated release/Market/parent facts, and these assert the bytes
        // in hand are the bytes that were authenticated. The digest equality is
        // the load-bearing one -- without it a caller could authenticate one
        // request and hand the composer another of the same width.
        let input = request.input();
        if input.release_set != parent.release_set
            || input.market != parent.market
            || dclutch_sha256_adapter::digest(family_request) != parent.parent_request_digest
        {
            return Err(TradingSbfError::Content.into());
        }
        Some(
            ClaimsExternalOnceV3::new(
                family_request,
                u16::try_from(fixed_account_count).map_err(|_| TradingSbfError::Content)?,
            )
            .map_err(|_| TradingSbfError::Content)?,
        )
    } else {
        None
    };
    HeapBoxV3::new(
        ClaimsCompositionV3::decode_selected_with_external(
            effect.base(),
            Some(effect.successor),
            tail_count,
            scalars,
            identities,
            request_bank,
            family_request,
            parent,
            external,
        )
        .map_err(|error| {
            // Seven conjunct families behind one `Content`, and the callee
            // already knows which. The wire still carries one code -- these are
            // genuinely one accusation, "the Claims composition this bundle
            // declares is not one this program admits" -- so the distinction
            // goes where a reader looks first. Localizing it by hand cost this
            // lane one instrumented build on 2026-09-02.
            solana_program::log::sol_log("dclutch-hot:claims-composition");
            solana_program::log::sol_log_64(
                u64::from(claims_composition_tag_v3(error)),
                0,
                0,
                0,
                0,
            );
            TradingSbfError::Content
        })?,
    )
}

/// Stable tag for one [`ClaimsCompositionErrorV3`], for the refusing log only.
///
/// Not a refusal code and never on the wire: the enum is a host-side type with
/// no registered discriminants, and giving it any here would be a second
/// authority for something `dclutch-refusal-registry` owns.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
const fn claims_composition_tag_v3(error: ClaimsCompositionErrorV3) -> u8 {
    match error {
        ClaimsCompositionErrorV3::EffectProgram => 1,
        ClaimsCompositionErrorV3::Route => 2,
        ClaimsCompositionErrorV3::Order => 3,
        ClaimsCompositionErrorV3::ParentBinding => 4,
        ClaimsCompositionErrorV3::AdmissionJoin => 5,
        ClaimsCompositionErrorV3::CloseJoin => 6,
        ClaimsCompositionErrorV3::MissingAffine => 7,
    }
}

/// Everything the preflight walk and the execution walk BOTH resolve, resolved
/// once for the pair.
///
/// The two walks are made over the same Effect, at the same registers, against
/// the same downgraded account vector, for the same release set, and nothing
/// between them can change any of those: the child routes have not run yet at
/// preflight, and the commit phase re-enters with the identical arguments. Each
/// walk was therefore paying, in full, for a decode whose answer the other walk
/// had already computed:
///
/// | resolved | per walk | across the pair |
/// | --- | ---: | ---: |
/// | Claims composition (`ClaimsCompositionV3::decode_selected_with_witness`) | 41,584 CU | 83,168 CU |
/// | three role programs (`selected_role_programs_v3`) | 36,562 CU | 73,124 CU |
///
/// Sharing removes the SECOND occurrence of both, which is the one the
/// execution walk makes -- 78,146 CU and 1,465 bytes of bump-allocated heap
/// that the run had no room for, since the execution walk reaches them only
/// after the six lifecycle account creations.
///
/// It needs no V3/V4 unification: both walks already resolve these from the
/// same V4-shifted inputs, and the V3-unshifted `effect.base()` the per-role
/// composition preparers take is untouched here.
pub(super) struct ChildWalkResolutionV3<'request, 'info> {
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    claims_composition: Option<HeapBoxV3<ClaimsCompositionV3<'request>>>,
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    roles: SelectedRoleProgramsV3<'info>,
    // The family-less outer profile resolves neither, and still has to name the
    // two lifetimes the walks are parameterised over.
    lifetimes: core::marker::PhantomData<(&'request [u8], &'info [u8])>,
}

/// The preflight walk's caller-authority derivations, in walk order.
///
/// THE CURSOR. Both child walks enumerate `route in 0..route_count` and
/// `invocation in 0..invocation_count(route)` from the same Effect at the same
/// registers, and both classify each invocation by the `role` of the same
/// resolution -- so the subsequence of invocations that need a caller authority
/// (every role but Claims, which derives once in its own walk and never twice)
/// is the same sequence in the same order for both. One byte per entry is the
/// whole carrier: the execution walk reads them back in order and
/// [`Self::exhausted`] refuses unless it consumed exactly what the preflight
/// produced, which makes "same order" a checked claim and not an assumption.
///
/// **Inline, and never heap.** This value is live from the preflight walk to
/// the last child CPI, which spans the exact phases where the run has the least
/// heap: the peak is 29,895 bytes of 32,768 at `commit-root`, 2,873 to spare.
/// A `Vec` here cost 16 of those bytes measured -- one byte of payload and
/// fifteen of alignment padding pushed onto the NEXT allocation -- because on
/// the SBF bump allocator every live allocation has a 16-byte floor and nothing
/// is ever given back. So the bumps live on
/// `execute_authenticated_hot_v3`'s own frame, which has room, and the boxed
/// commit plan holds a shared reference to them like every other borrowed
/// register bank it carries.
///
/// [`INLINE_CHILD_CALLER_BUMPS_V4`] is a MEASURED-PROFILE bound, not a protocol
/// one, and it is not a refusal: a walk with more child invocations than fit
/// simply stops recording, and every invocation past the boundary derives its
/// own authority exactly as it did before this existed. The saving degrades;
/// nothing else changes. Today's widest executing bundle records one.
const INLINE_CHILD_CALLER_BUMPS_V4: usize = 8;

pub(super) struct ChildCallerBumpsV4 {
    bumps: [u8; INLINE_CHILD_CALLER_BUMPS_V4],
    len: usize,
    /// Set once the walk produced more entries than fit. Both walks then agree
    /// to derive from the boundary on, so the cursor stays honest.
    overflowed: bool,
    /// Widest exact child instruction wire authenticated during the same
    /// mutation-free preflight walk.
    max_wire_bytes: usize,
    /// Exact number of child invocations authenticated by preflight.
    total_invocations: usize,
}

impl Default for ChildCallerBumpsV4 {
    fn default() -> Self {
        Self {
            bumps: [0; INLINE_CHILD_CALLER_BUMPS_V4],
            len: 0,
            overflowed: false,
            max_wire_bytes: 0,
            total_invocations: 0,
        }
    }
}

impl ChildCallerBumpsV4 {
    fn record_wire_bytes(&mut self, bytes: usize) {
        self.max_wire_bytes = self.max_wire_bytes.max(bytes);
    }

    fn record_invocations(&mut self, count: u32) -> Result<(), ProgramError> {
        self.total_invocations = self
            .total_invocations
            .checked_add(usize::try_from(count).map_err(|_| TradingSbfError::Content)?)
            .ok_or(TradingSbfError::Content)?;
        Ok(())
    }

    /// Record what the preflight walk derived for the next invocation in order.
    fn record(&mut self, bump: u8) -> Result<(), ProgramError> {
        match self.bumps.get_mut(self.len) {
            Some(slot) => {
                *slot = bump;
                self.len = self.len.checked_add(1).ok_or(TradingSbfError::Content)?;
            }
            None => self.overflowed = true,
        }
        Ok(())
    }

    /// The bump the preflight walk derived for the invocation at `cursor`.
    ///
    /// `None` past the inline boundary, which tells the execution walk to
    /// derive canonically for itself rather than refuse.
    fn at(&self, cursor: usize) -> Option<u8> {
        if cursor < self.len {
            self.bumps.get(cursor).copied()
        } else {
            None
        }
    }

    /// Refuse unless the execution walk consumed exactly the preflight's set.
    ///
    /// Vacuous once the preflight overflowed, because past that boundary the
    /// two walks are deriving independently and there is no set to consume.
    fn exhausted(&self, cursor: usize) -> Result<(), ProgramError> {
        if self.overflowed || cursor == self.len {
            Ok(())
        } else {
            Err(TradingSbfError::Content.into())
        }
    }
}

/// Resolve, once, what both child walks would each have resolved for themselves.
///
/// Deliberately ONE out-of-line function returning a `Box`: the decoded
/// composition and the three `AccountInfo` handles are about 200 bytes, and
/// `process_hot_execution_v3` is already near the SBPF v0 4,096-byte static
/// frame bound. Only the box pointer crosses back. The walks read through it
/// rather than holding the handles, which takes those same bytes back OFF
/// `execute_child_routes_v3`'s frame, where four frame-overwrite diagnostics
/// were reported the last time it held three decoded addresses of its own.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn resolve_child_walk_v3<'request, 'accounts, 'info>(
    frame: HotFrameV3<'accounts, 'info>,
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, 'accounts, 'info>,
    aliases: &[usize],
    request_bank: &'request [u8],
    family_request: &'request [u8],
    request_digest: [u8; 32],
    envelope: HotExecutionEnvelopeV3,
    child_programs: Option<AuthenticatedChildProgramsV3>,
) -> Result<HeapBoxV3<ChildWalkResolutionV3<'request, 'info>>, ProgramError> {
    #[cfg(not(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    )))]
    let _ = (
        frame,
        effect,
        tail_count,
        scalars,
        identities,
        effect_accounts,
        aliases,
        request_bank,
        family_request,
        request_digest,
        envelope,
        child_programs,
    );
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let required_roles = active_roles_v3(effect, tail_count, scalars, identities)?;
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let claims_composition = if required_roles.claims {
        Some(decode_claims_composition_boxed_v3(
            effect,
            tail_count,
            scalars,
            identities,
            request_bank,
            family_request,
            ClaimsCompositionParentV3 {
                release_set: envelope.release_set(),
                market: envelope.market(),
                generation: envelope.generation(),
                parent_request_digest: request_digest,
            },
        )?)
    } else {
        None
    };
    hot_heap_mark!("shared-claims-composition");
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let roles = if effect.route_count() == 0 {
        SelectedRoleProgramsV3 {
            claims: None,
            custody: None,
            #[cfg(feature = "families")]
            resolution: None,
        }
    } else {
        selected_role_programs_v3(
            frame,
            effect_accounts,
            aliases,
            envelope.release_set(),
            child_programs,
            required_roles,
        )?
    };
    hot_heap_mark!("shared-role-programs");
    let resolution = ChildWalkResolutionV3 {
        #[cfg(any(
            feature = "families",
            feature = "series-family",
            feature = "dealer-family"
        ))]
        claims_composition,
        #[cfg(any(
            feature = "families",
            feature = "series-family",
            feature = "dealer-family"
        ))]
        roles,
        lifetimes: core::marker::PhantomData,
    };
    HeapBoxV3::new(resolution)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn preflight_child_routes_v3<'accounts, 'info>(
    program_id: &Pubkey,
    frame: HotFrameV3<'accounts, 'info>,
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, 'accounts, 'info>,
    request_bank: &[u8],
    family_request: &[u8],
    request_digest: [u8; 32],
    envelope: HotExecutionEnvelopeV3,
    capability_program_set: [u8; 32],
    selected_capability_program: [u8; 32],
    aliases: &[usize],
    // Resolved once for both walks; see `ChildWalkResolutionV3`.
    resolved: &ChildWalkResolutionV3<'_, 'info>,
    // Folded out of the one walk over this Effect that
    // `require_local_effect_discipline_v5` already makes, at these exact
    // registers. `None` only when that walk saw no route to answer for.
    participation: Option<&mut [CoordinateParticipationV3]>,
    authenticated_series_expiry_replay: bool,
    authenticated_series_expiry_rent_credit: [u8; 32],
) -> Result<ChildCallerBumpsV4, ProgramError> {
    #[cfg(not(feature = "families"))]
    let _ = (
        request_digest,
        capability_program_set,
        selected_capability_program,
    );
    let mut caller_bumps = ChildCallerBumpsV4::default();
    if effect.route_count() == 0 {
        return Ok(caller_bumps);
    }
    let participation = participation.ok_or(TradingSbfError::Content)?;
    if participation.len() != aliases.len() {
        return Err(TradingSbfError::Content.into());
    }
    let successor_account_count = effect
        .successor
        .account_count(tail_count, scalars)
        .map_err(|_| TradingSbfError::Content)?;
    if successor_account_count != effect_accounts.len() {
        return Err(TradingSbfError::Content.into());
    }
    hot_heap_mark!("pf-enter");
    #[cfg(not(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    )))]
    let _ = resolved;
    // Both the Claims composition and the three role carriers were resolved
    // once for this walk AND the execution walk; see `ChildWalkResolutionV3`.
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let claims_composition = resolved.claims_composition.as_deref();
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let claims_program = resolved.roles.claims.as_ref();
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let custody_program = resolved.roles.custody.as_ref();
    #[cfg(feature = "families")]
    let resolution_program = resolved.roles.resolution.as_ref();

    hot_cu_checkpoint!("pf-role-programs");
    // ONE frame for the whole preflight walk, for the same reason the execution
    // walk has one: a per-invocation frame is a per-invocation charge on an
    // allocator that never gives one back.
    let mut preflight_frame: Vec<AccountInfo<'info>> = Vec::new();
    let mut preflight_wire: Vec<u8> = Vec::new();
    let mut route = 0_u16;
    // Position of the next child invocation in the whole walk, route-major.
    // The execution walk counts the same way, so both agree on which hint slot
    // an invocation owns without carrying the assignment between them.
    let mut child_ordinal = 0_usize;
    while route < effect.route_count() {
        let count = effect
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?;
        caller_bumps.record_invocations(count)?;
        let mut invocation_index = 0_u32;
        while invocation_index < count {
            let caller_hint = child_caller_hint_v1(envelope, child_ordinal);
            let invocation = effect
                .resolved_invocation(route, invocation_index, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Content)?;
            hot_heap_mark!("pf-invocation");
            hot_cu_checkpoint!("pf-invocation-resolved");
            require_chain_receipt_width_v3(effect.base(), invocation)?;
            require_no_common_projection_child_accounts_v3(invocation)?;
            let allowed_local_overlap = if let Some(root) =
                fractional_local_root_overlap_v3(invocation, request_bank, family_request, aliases)?
            {
                AllowedLocalOverlapV3::FractionalRoot(root)
            } else {
                series_expiry_local_replay_overlap_v1(
                    effect,
                    route,
                    invocation_index,
                    invocation,
                    tail_count,
                    scalars,
                    request_bank,
                    family_request,
                    aliases,
                    participation,
                    effect_accounts,
                    authenticated_series_expiry_replay,
                    authenticated_series_expiry_rent_credit,
                    CoreCompositionParentV3 {
                        release_set: envelope.release_set(),
                        market: envelope.market(),
                        generation: envelope.generation(),
                        trading_program: program_id.to_bytes(),
                    },
                )?
            };
            record_child_reach_and_require_disjoint_from_local(
                invocation,
                aliases,
                participation,
                allowed_local_overlap,
            )?;
            match invocation.role {
                FixedRole::Core => {
                    caller_bumps.record(preflight_core_route_v3(
                        program_id,
                        effect.base(),
                        route,
                        invocation_index,
                        invocation,
                        successor_account_count,
                        BorrowedRouteRangesV4::new(
                            effect.successor,
                            route,
                            tail_count,
                            scalars,
                            family_request,
                        ),
                        effect_accounts,
                        request_bank,
                        &mut preflight_frame,
                        &mut preflight_wire,
                        frame.core_program,
                        CoreCompositionParentV3 {
                            release_set: envelope.release_set(),
                            market: envelope.market(),
                            generation: envelope.generation(),
                            trading_program: program_id.to_bytes(),
                        },
                        caller_hint,
                    )?)?;
                    let suffix = usize::try_from(receipt_dependency_width_v3(invocation))
                        .map_err(|_| TradingSbfError::Content)?;
                    caller_bumps.record_wire_bytes(
                        preflight_wire
                            .len()
                            .checked_add(suffix)
                            .ok_or(TradingSbfError::Content)?,
                    );
                }
                FixedRole::Claims => {
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    {
                        // ONE conjunct is this walk's to refuse, and the
                        // comment here used to promise three. The 2026-09-01
                        // wall was real: the route reached this arm for the
                        // first time and published the same `Content` as 2,124
                        // other sites, so the distinction goes to a validator
                        // log, which is where a reader looks first and which is
                        // all the wire's one code leaves room for. But the
                        // other two conjuncts are not sized here. The role is
                        // this match arm. "This is not a child route this
                        // composition owns" belongs to
                        // `claims_composition_v3::execute_claims_route_v3`,
                        // which refuses it on the way to the CPI; asking it
                        // again in the sizing walk would be a second authority
                        // for one fact, and the two walks already share ONE
                        // resolution so they cannot disagree about the inputs.
                        //
                        // Both resolutions are still REACHED, and that is not
                        // decoration: an absent composition or role carrier
                        // refuses in this walk instead of faulting in the
                        // executing one. Neither value is read.
                        claims_composition.ok_or(TradingSbfError::Content)?;
                        claims_program.ok_or(TradingSbfError::Release)?;
                        if invocation_index != 0 {
                            solana_program::log::sol_log(
                                "dclutch-hot-claims-preflight: nonzero invocation index",
                            );
                            return Err(TradingSbfError::Content.into());
                        }
                        caller_bumps.record_wire_bytes(claims_child_wire_capacity_v3(
                            invocation,
                            request_bank,
                            BorrowedRouteRangesV4::new(
                                effect.successor,
                                route,
                                tail_count,
                                scalars,
                                family_request,
                            ),
                        )?);
                    }
                    #[cfg(not(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    )))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Custody => {
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    caller_bumps.record(preflight_custody_route_v3(
                        program_id,
                        successor_account_count,
                        invocation,
                        effect_accounts,
                        request_bank,
                        &mut preflight_frame,
                        custody_program.ok_or(TradingSbfError::Release)?,
                        CustodyCompositionParentV3 {
                            release_set: envelope.release_set(),
                            market: envelope.market(),
                            generation: envelope.generation(),
                            parent_request_digest: request_digest,
                            trading_program: program_id.to_bytes(),
                            child_relay: envelope.bump_hints().child_relay,
                        },
                        caller_hint,
                    )?)?;
                    // Plus the three bytes `execute_custody_route_v3` appends:
                    // the caller-authority bump, and the replay and transfer
                    // authority bumps the child reads instead of searching for
                    // them. Sized here so the walk's single wire buffer is
                    // still bought exactly once.
                    caller_bumps.record_wire_bytes(
                        invocation
                            .request_len
                            .checked_add(CUSTODY_BUMP_RELAY_BYTES_V1)
                            .ok_or(TradingSbfError::Content)?,
                    );
                    #[cfg(not(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    )))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Resolution => {
                    #[cfg(feature = "families")]
                    caller_bumps.record(preflight_resolution_route_v3(
                        program_id,
                        effect.base(),
                        route,
                        invocation_index,
                        tail_count,
                        scalars,
                        identities,
                        effect_accounts,
                        request_bank,
                        family_request,
                        &mut preflight_frame,
                        &mut preflight_wire,
                        resolution_program.ok_or(TradingSbfError::Release)?,
                        ResolutionCompositionParentV3 {
                            release_set: envelope.release_set(),
                            market: envelope.market(),
                            generation: envelope.generation(),
                            parent_request_digest: request_digest,
                            trading_program: program_id.to_bytes(),
                            capability_program_set,
                            selected_capability_program,
                            activation_account: frame.activation_cache.key.to_bytes(),
                        },
                        caller_hint,
                    )?)?;
                    caller_bumps.record_wire_bytes(preflight_wire.len());
                    #[cfg(not(feature = "families"))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
            }
            hot_cu_checkpoint!("pf-invocation-preflighted");
            invocation_index = invocation_index
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?;
            child_ordinal = child_ordinal
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?;
        }
        route = route.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(caller_bumps)
}

struct ChildExecutionStateV3<'info> {
    transcript: [u8; 32],
    receipt_bank: ChildReceiptBankV3,
    prior_receipt_bytes: Vec<u8>,
    // The walk's ONE set of child-CPI buffers. It lives inside this boxed
    // header rather than on the walk's frame because `execute_child_routes_v3`
    // is against the SBPF v0 4,096-byte static frame bound; only the box
    // pointer is on the frame either way.
    buffers: ChildInvocationBuffersV3<'info>,
    route: u16,
}

// The sole additional allocation introduced by the verifier-frame split is
// this bounded header. Receipt payloads already lived in Vec-backed storage
// before this split; no authenticated fact or commit authority moves from
// account data into the heap. The child-CPI buffer set added 128 bytes of
// header to save thousands of bytes of per-invocation duplication.
const _: [(); 216] = [(); core::mem::size_of::<ChildExecutionStateV3<'_>>()];

/// The caller's mined bump for the child invocation at `ordinal`, if any.
///
/// `Some` turns a composition's `prepare` from the walk that SEARCHES into the
/// walk that REPRODUCES: `child_caller_authority_v4` takes exactly this shape,
/// and every composition already compares the address it produces against the
/// account at coordinate 0. A wrong hint reproduces a different address and
/// refuses there, unchanged, so the hint is a memo and never an authority --
/// the same argument the module already makes for the preflight-to-execution
/// carry, now extended one step further out to the caller who mined it.
///
/// Zero, and every invocation past the end of the block, means the preflight
/// searches exactly as it used to.
const fn child_caller_hint_v1(
    envelope: HotExecutionEnvelopeV3,
    ordinal: usize,
) -> PreflightedCallerBumpV4 {
    let hints = envelope.bump_hints().child_caller;
    if ordinal < hints.len() {
        hot_bump_hint_v1(hints[ordinal])
    } else {
        None
    }
}

/// Read the next caller-authority bump the preflight walk derived.
///
/// Out of line and taking the cursor by reference so the execution walk's
/// frame carries one `usize`, not a borrow-checked cell: that walk is against
/// the SBPF v0 4,096-byte static frame bound.
#[inline(never)]
fn take_caller_bump_v4(
    bumps: &ChildCallerBumpsV4,
    cursor: &mut usize,
) -> Result<PreflightedCallerBumpV4, ProgramError> {
    let bump = bumps.at(*cursor);
    *cursor = cursor.checked_add(1).ok_or(TradingSbfError::Content)?;
    Ok(bump)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn execute_child_routes_v3<'accounts, 'info>(
    program_id: &Pubkey,
    frame: HotFrameV3<'accounts, 'info>,
    request_profile: RequestProfileKindV3<'_>,
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, 'accounts, 'info>,
    // Per-logical-coordinate representative table. It came BACK onto this
    // signature on 2026-09-01 for a second reader: the Claims route's
    // "the child frame carries the child program exactly once" check counted
    // raw keys, so a coordinate the frame's own profile declares an alias
    // counted as a second occurrence and refused every representation route.
    // It is a borrowed slice, so carrying it materialises nothing -- the
    // earlier removal was about not BUILDING the table here, and it is built
    // once for the projection either way.
    aliases: &[usize],
    request_bank: &[u8],
    family_request: &[u8],
    request_digest: [u8; 32],
    envelope: HotExecutionEnvelopeV3,
    capability_program_set: [u8; 32],
    selected_capability_program: [u8; 32],
    // Resolved once for this walk AND the preflight walk; see
    // `ChildWalkResolutionV3`. This is the whole reason the walk can reach a
    // child CPI at all: re-deriving them here cost 78,146 CU and 1,465 bytes
    // that the run does not have by the time it gets here. It also takes the
    // per-logical-coordinate alias table off this signature: the only reader
    // was the role-carrier resolution, which now happens once, elsewhere.
    shared: &ChildWalkResolutionV3<'_, 'info>,
    // What the preflight walk derived for each invocation's caller authority.
    // See `crate::child_authority_v4`: this walk reproduces those addresses
    // instead of searching for them a second time.
    caller_bumps: &ChildCallerBumpsV4,
    sparse_post_resource_verification: SparsePostResourceVerificationV3,
) -> Result<[u8; 32], ProgramError> {
    // The preflight walk's derivations, read back in the order it produced
    // them. See `ChildCallerBumpsV4`.
    let mut caller_bump_cursor = 0_usize;
    // Counted exactly as the preflight walk counts it, so the two walks assign
    // the same hint slot to the same invocation. This is NOT the cursor above:
    // that one advances only for the roles the preflight preflighted, and the
    // Claims route is derived here rather than there.
    let mut child_ordinal = 0_usize;
    #[cfg(not(feature = "families"))]
    let _ = (capability_program_set, selected_capability_program);
    #[cfg(not(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    )))]
    let _ = shared;
    let mut execution = Box::new(ChildExecutionStateV3 {
        transcript: hashv(&[CHILD_EXECUTION_DIGEST_DOMAIN_V3, &request_digest]).to_bytes(),
        receipt_bank: ChildReceiptBankV3::new(),
        prior_receipt_bytes: Vec::new(),
        buffers: ChildInvocationBuffersV3::new(),
        route: 0,
    });
    execution
        .buffers
        .reserve_wire_exact(caller_bumps.max_wire_bytes)?;
    execution
        .receipt_bank
        .reserve_total(caller_bumps.total_invocations)?;
    hot_heap_mark!("child-execution-state");
    if effect.route_count() == 0 {
        return Ok(execution.transcript);
    }
    let successor_account_count = effect
        .successor
        .account_count(tail_count, scalars)
        .map_err(|_| TradingSbfError::Content)?;
    if successor_account_count != effect_accounts.len() {
        return Err(TradingSbfError::Content.into());
    }
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let claims_composition = shared.claims_composition.as_deref();
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let claims_program = shared.roles.claims.as_ref();
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let custody_program = shared.roles.custody.as_ref();
    #[cfg(feature = "families")]
    let resolution_program = shared.roles.resolution.as_ref();

    while execution.route < effect.route_count() {
        let route = execution.route;
        let count = effect
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?;
        let mut invocation = 0_u32;
        while invocation < count {
            let resolved = effect
                .resolved_invocation(route, invocation, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Content)?;
            // Split the boxed header once. A single dependency can stay
            // borrowed directly from the authenticated receipt bank while the
            // disjoint CPI buffer field is mutated, avoiding a second full
            // return-data allocation on the bump heap. Multiple dependencies
            // retain the exact ordered concatenation path.
            let ChildExecutionStateV3 {
                transcript,
                receipt_bank,
                prior_receipt_bytes,
                buffers,
                route: _,
            } = &mut *execution;
            buffers.producer = if receipt_bank
                .len()
                .checked_add(1)
                .is_some_and(|next| next == caller_bumps.total_invocations)
            {
                Pubkey::default()
            } else {
                Pubkey::new_from_array([1; 32])
            };
            prior_receipt_bytes.clear();
            let borrows_single_receipt = resolved.receipt_dependencies.len() == 1;
            let mut single_receipt = None;
            let mut dependency_index = 0_u16;
            while dependency_index < resolved.receipt_dependencies.len() {
                let dependency = effect
                    .resolved_receipt_dependency(resolved.receipt_dependencies, dependency_index)
                    .map_err(|_| TradingSbfError::Content)?;
                let dependency_program = match dependency.producer_role {
                    FixedRole::Core => frame.core_program,
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    FixedRole::Claims => claims_program.ok_or(TradingSbfError::Release)?,
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    FixedRole::Custody => custody_program.ok_or(TradingSbfError::Release)?,
                    #[cfg(feature = "families")]
                    FixedRole::Resolution => resolution_program.ok_or(TradingSbfError::Release)?,
                    #[cfg(not(feature = "families"))]
                    _ => return Err(TradingSbfError::UnsupportedContent.into()),
                };
                let producer_invocation = effect
                    .resolved_invocation(
                        dependency.producer_route,
                        dependency.producer_invocation,
                        tail_count,
                        scalars,
                        identities,
                    )
                    .map_err(|_| TradingSbfError::Content)?;
                let expected_provenance = child_receipt_provenance_v4(
                    producer_invocation,
                    BorrowedRouteRangesV4::new(
                        effect.successor,
                        dependency.producer_route,
                        tail_count,
                        scalars,
                        family_request,
                    ),
                    dependency.producer_role,
                    dependency.producer_route,
                    dependency.producer_invocation,
                    dependency_program.key,
                    envelope.release_set(),
                    envelope.market(),
                    envelope.generation(),
                    request_digest,
                    request_bank,
                    family_request,
                )?;
                let receipt = receipt_bank
                    .resolve(Some(dependency), Some(expected_provenance))?
                    .ok_or(TradingSbfError::Transition)?;
                if borrows_single_receipt {
                    if single_receipt.replace(receipt).is_some() {
                        return Err(TradingSbfError::Content.into());
                    }
                } else {
                    // EXACT, not amortised. `try_reserve` grows by doubling,
                    // and on an allocator that never gives a block back the
                    // doubling slack would be permanent heap.
                    prior_receipt_bytes
                        .try_reserve_exact(receipt.len())
                        .map_err(|_| TradingSbfError::HeapExhausted)?;
                    prior_receipt_bytes.extend_from_slice(receipt);
                }
                dependency_index = dependency_index
                    .checked_add(1)
                    .ok_or(TradingSbfError::Content)?;
            }
            hot_heap_mark!("child-dependencies");
            hot_cu_checkpoint!("cw-dependencies");
            let prior_receipt = if let Some(receipt) = single_receipt {
                Some(receipt)
            } else if prior_receipt_bytes.is_empty() {
                None
            } else {
                Some(prior_receipt_bytes.as_slice())
            };
            let (role, child_digest, child_program, receiptless) = match resolved.role {
                FixedRole::Core => {
                    let executed = execute_core_route_v3(
                        program_id,
                        effect.base(),
                        resolved,
                        route,
                        invocation,
                        successor_account_count,
                        BorrowedRouteRangesV4::new(
                            effect.successor,
                            route,
                            tail_count,
                            scalars,
                            family_request,
                        ),
                        effect_accounts,
                        request_bank,
                        prior_receipt,
                        buffers,
                        frame.core_program,
                        CoreCompositionParentV3 {
                            release_set: envelope.release_set(),
                            market: envelope.market(),
                            generation: envelope.generation(),
                            trading_program: program_id.to_bytes(),
                        },
                        take_caller_bump_v4(caller_bumps, &mut caller_bump_cursor)?,
                    )?;
                    (
                        FixedRole::Core,
                        executed.digest(),
                        frame.core_program,
                        executed.receiptless(),
                    )
                }
                FixedRole::Claims => {
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    {
                        (
                            FixedRole::Claims,
                            execute_claims_route_digest_v3(
                                program_id,
                                successor_account_count,
                                claims_composition
                                    .copied()
                                    .ok_or(TradingSbfError::Content)?,
                                route,
                                invocation,
                                resolved,
                                effect_accounts,
                                aliases,
                                request_bank,
                                BorrowedRouteRangesV4::new(
                                    effect.successor,
                                    route,
                                    tail_count,
                                    scalars,
                                    family_request,
                                ),
                                prior_receipt,
                                buffers,
                                claims_program.ok_or(TradingSbfError::Release)?,
                                sparse_post_resource_verification,
                                child_caller_hint_v1(envelope, child_ordinal),
                            )?,
                            claims_program.ok_or(TradingSbfError::Release)?,
                            false,
                        )
                    }
                    #[cfg(not(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    )))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Custody => {
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    {
                        let digest = execute_custody_route_v3(
                            program_id,
                            successor_account_count,
                            route,
                            invocation,
                            resolved,
                            effect_accounts,
                            request_bank,
                            prior_receipt,
                            buffers,
                            custody_program.ok_or(TradingSbfError::Release)?,
                            CustodyCompositionParentV3 {
                                release_set: envelope.release_set(),
                                market: envelope.market(),
                                generation: envelope.generation(),
                                parent_request_digest: request_digest,
                                trading_program: program_id.to_bytes(),
                                child_relay: envelope.bump_hints().child_relay,
                            },
                            take_caller_bump_v4(caller_bumps, &mut caller_bump_cursor)?,
                        )?;
                        (
                            FixedRole::Custody,
                            digest,
                            custody_program.ok_or(TradingSbfError::Release)?,
                            false,
                        )
                    }
                    #[cfg(not(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    )))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Resolution => {
                    #[cfg(feature = "families")]
                    {
                        let digest = execute_resolution_route_v3(
                            program_id,
                            effect.base(),
                            route,
                            invocation,
                            tail_count,
                            scalars,
                            identities,
                            effect_accounts,
                            request_bank,
                            family_request,
                            prior_receipt,
                            buffers,
                            resolution_program.ok_or(TradingSbfError::Release)?,
                            ResolutionCompositionParentV3 {
                                release_set: envelope.release_set(),
                                market: envelope.market(),
                                generation: envelope.generation(),
                                parent_request_digest: request_digest,
                                trading_program: program_id.to_bytes(),
                                capability_program_set,
                                selected_capability_program,
                                activation_account: frame.activation_cache.key.to_bytes(),
                            },
                            take_caller_bump_v4(caller_bumps, &mut caller_bump_cursor)?,
                        )?;
                        (
                            FixedRole::Resolution,
                            digest,
                            resolution_program.ok_or(TradingSbfError::Release)?,
                            false,
                        )
                    }
                    #[cfg(not(feature = "families"))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
            };
            hot_cu_checkpoint!("cw-child-returned");
            hot_heap_mark!("child-invoked");
            // The return-data syscall was read ONCE, by the composition that
            // verified the receipt against its own request. It used to be read
            // a second time here, into a second vector this allocator never
            // gave back -- 938 bytes across the two child CPIs of the canonical
            // Direct bundle. The bytes the composition already owns are moved
            // straight into the bank instead.
            let producer = buffers.producer;
            let receipt_bytes = buffers.take_returned();
            hot_heap_mark!("child-return-data");
            if producer != *child_program.key {
                return Err(TradingSbfError::Transition.into());
            }
            if receiptless {
                // The typed Core composition has already proved this is the
                // permissionless Series permit-expiry request, invoked the
                // activated Core with no signer seeds, required an absent
                // return channel, and refused every Effect dependency on this
                // invocation. Successful execution therefore contributes its
                // dedicated digest to the transcript but no invented receipt
                // payload to the dependency bank.
                if role != FixedRole::Core || !receipt_bytes.is_empty() {
                    return Err(TradingSbfError::Transition.into());
                }
                *transcript = hashv(&[
                    CHILD_EXECUTION_DIGEST_DOMAIN_V3,
                    &*transcript,
                    &[fixed_role_tag_v3(role)],
                    &route.to_le_bytes(),
                    &invocation.to_le_bytes(),
                    child_program.key.as_ref(),
                    &child_digest,
                ])
                .to_bytes();
                hot_heap_mark!("child-receiptless");
                invocation = invocation.checked_add(1).ok_or(TradingSbfError::Content)?;
                child_ordinal = child_ordinal
                    .checked_add(1)
                    .ok_or(TradingSbfError::Content)?;
                continue;
            }
            let borrowed_ranges = BorrowedRouteRangesV4::new(
                effect.successor,
                route,
                tail_count,
                scalars,
                family_request,
            );
            require_borrowed_witness_receipt_v3(
                request_profile,
                borrowed_ranges.count()?,
                role,
                &receipt_bytes,
            )?;
            let provenance = child_receipt_provenance_v4(
                resolved,
                borrowed_ranges,
                role,
                route,
                invocation,
                child_program.key,
                envelope.release_set(),
                envelope.market(),
                envelope.generation(),
                request_digest,
                request_bank,
                family_request,
            )?;
            let receipt_kind: [u8; 8] = receipt_bytes
                .get(..8)
                .ok_or(TradingSbfError::Transition)?
                .try_into()
                .map_err(|_| TradingSbfError::Transition)?;
            let receipt_digest = hash(&receipt_bytes).to_bytes();
            receipt_bank.record_exact(
                role,
                route,
                invocation,
                producer,
                provenance.context_digest,
                provenance.request_kind,
                provenance.request_digest,
                receipt_kind,
                receipt_bytes,
            )?;
            *transcript = hashv(&[
                CHILD_EXECUTION_DIGEST_DOMAIN_V3,
                &*transcript,
                &[fixed_role_tag_v3(role)],
                &route.to_le_bytes(),
                &invocation.to_le_bytes(),
                child_program.key.as_ref(),
                &receipt_digest,
                &child_digest,
            ])
            .to_bytes();
            hot_cu_checkpoint!("cw-banked");
            hot_heap_mark!("child-banked");
            invocation = invocation.checked_add(1).ok_or(TradingSbfError::Content)?;
            child_ordinal = child_ordinal
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?;
        }
        execution.route = route.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    caller_bumps.exhausted(caller_bump_cursor)?;
    Ok(execution.transcript)
}

fn fixed_role_tag_v3(role: FixedRole) -> u8 {
    match role {
        FixedRole::Core => 0,
        FixedRole::Claims => 1,
        FixedRole::Resolution => 3,
        FixedRole::Custody => 4,
    }
}

#[allow(clippy::too_many_arguments)]
fn child_receipt_provenance_v4(
    invocation: dclutch_vm::effect::v3::ResolvedInvocationV3,
    borrowed_ranges: BorrowedRouteRangesV4<'_, '_, '_>,
    role: FixedRole,
    route: u16,
    invocation_index: u32,
    child_program: &Pubkey,
    release_set: [u8; 32],
    market: [u8; 32],
    generation: u64,
    parent_request_digest: [u8; 32],
    request_bank: &[u8],
    family_request: &[u8],
) -> Result<ExpectedReceiptProvenanceV4, ProgramError> {
    let request_end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    let child_request = request_bank
        .get(invocation.request_offset..request_end)
        .ok_or(TradingSbfError::Content)?;
    let borrowed_request = invocation
        .borrowed_witness
        .map(|witness| {
            witness
                .slice(family_request)
                .map_err(|_| TradingSbfError::Content)
        })
        .transpose()?;
    let range_count = borrowed_ranges.count()?;
    if range_count != 0 && borrowed_request.is_some() {
        return Err(TradingSbfError::Content.into());
    }
    let request_kind_source = if child_request.len() >= 8 {
        child_request
    } else if child_request.is_empty() {
        if let Some(request) = borrowed_request {
            request
        } else if range_count != 0 {
            borrowed_ranges.range(0)?
        } else {
            return Err(TradingSbfError::Content.into());
        }
    } else {
        return Err(TradingSbfError::Content.into());
    };
    let request_kind = request_kind_source
        .get(..8)
        .ok_or(TradingSbfError::Content)?
        .try_into()
        .map_err(|_| TradingSbfError::Content)?;
    // Domain ‖ 0x00 ‖ presence tag ‖ u32_le(request len) ‖ u32_le(witness len)
    // ‖ request ‖ witness — the `shadow_digest_v3` framing convention, with the
    // lengths hoisted ahead of the variable fields so `hashv` never has to see
    // a concatenation it cannot re-split.
    //
    // `hashv` concatenates its parts and frames nothing. Digesting the request
    // and the borrowed witness bare therefore committed only to their
    // concatenation: any other split of the same bytes hashes identically, and
    // a witnessless request collides with every pair that spells it. Both
    // preimages are attacker-shaped — the Effect program chooses
    // `request_offset`/`request_len` and the borrowed-witness range — and this
    // digest is exactly what binds a resolved receipt to the request that
    // produced it, so a collision here lets one invocation's receipt satisfy
    // another's declared dependency.
    let request_digest = if range_count == 0 {
        // Exact compatibility for every existing zero-range bundle and the
        // legacy single-witness grammar.
        child_request_digest_v4(child_request, borrowed_request)?
    } else {
        child_request_digest_v5(child_request, range_count, |ordinal| {
            borrowed_ranges.range(ordinal).ok()
        })?
    };
    let context_digest = hashv(&[
        CHILD_RECEIPT_CONTEXT_DOMAIN_V4,
        &release_set,
        &market,
        &generation.to_le_bytes(),
        &parent_request_digest,
        &[fixed_role_tag_v3(role)],
        &route.to_le_bytes(),
        &invocation_index.to_le_bytes(),
        child_program.as_ref(),
        &request_digest,
    ])
    .to_bytes();
    Ok(ExpectedReceiptProvenanceV4 {
        context_digest,
        request_kind,
        request_digest,
    })
}

#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
fn active_roles_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<RequiredRolesV3, ProgramError> {
    let mut required = RequiredRolesV3 {
        claims: false,
        custody: false,
        resolution: false,
    };
    let mut route = 0_u16;
    while route < effect.route_count() {
        let route_program = effect.route(route).map_err(|_| TradingSbfError::Content)?;
        if effect
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?
            != 0
        {
            match route_program.role() {
                FixedRole::Claims => required.claims = true,
                FixedRole::Custody => required.custody = true,
                FixedRole::Resolution => required.resolution = true,
                FixedRole::Core => {}
            }
        }
        route = route.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(required)
}

pub(super) fn mark_local_mutation(
    effect: ResolvedEffectV3,
    aliases: &[usize],
    output: &mut [CoordinateParticipationV3],
) -> Result<(), ProgramError> {
    let coordinates = match effect {
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
        let representative = *aliases.get(coordinate).ok_or(TradingSbfError::Content)?;
        output
            .get_mut(representative)
            .ok_or(TradingSbfError::Content)?
            .mark_local_mutation();
    }
    Ok(())
}

/// Refuse a child invocation that reaches a coordinate this Effect's own local
/// operations mutate, and record that it reached the rest.
///
/// The coordinates are ENUMERATED, never COLLECTED. This used to gather them
/// into a `Vec<usize>` and then walk it, once per invocation, on an allocator
/// that gives nothing back -- 184 bytes for a Claims frame and again for a
/// Custody one, in the preflight walk, on top of the doubling ladder the
/// second `extend` pays for. The check is per coordinate and order-independent,
/// so the window walk answers directly and the first offending coordinate
/// refuses in exactly the same place it did before.
/// What one logical coordinate participates in during this execution.
///
/// Two facts, one bank, written by two walks that are not allowed to disagree
/// about a coordinate. The Effect projection marks every coordinate its own
/// local operations mutate; the preflight child walk marks every coordinate a
/// declared child invocation reaches. `record_child_reach_and_require_disjoint_
/// from_local` refuses a coordinate carrying both, outside the closed
/// `AllowedLocalOverlapV3` set.
///
/// That refusal is what the commit's lamport authority rests on. A child-reached
/// coordinate is never a local-effect target, so `output_lamports` for it is its
/// own observed PRESTATE -- the plan holds no opinion about it at all -- and
/// writing that plan back could only ever REVERT what the child did. Keeping the
/// two facts in one bank is not a space saving: it is what makes it impossible
/// for the walk that proves disjointness and the walk that relies on it to be
/// looking at different coordinates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CoordinateParticipationV3(u8);

impl CoordinateParticipationV3 {
    const LOCAL_MUTATION: u8 = 1;
    const CHILD_REACH: u8 = 2;

    /// No Effect declares child routes, so the local plan is the sole authority.
    pub(super) const PLAN_IS_SOLE_AUTHORITY: Self = Self(Self::LOCAL_MUTATION);

    pub(super) const fn locally_mutated(self) -> bool {
        self.0 & Self::LOCAL_MUTATION != 0
    }

    pub(super) const fn child_reached(self) -> bool {
        self.0 & Self::CHILD_REACH != 0
    }

    pub(super) fn mark_local_mutation(&mut self) {
        self.0 |= Self::LOCAL_MUTATION;
    }

    pub(super) fn mark_child_reach(&mut self) {
        self.0 |= Self::CHILD_REACH;
    }
}

/// What the commit may do to one coordinate's lamports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CommittedLamportsV3 {
    /// This Effect declared the movement, so its plan is what lands.
    Apply,
    /// The account already holds the planned balance; there is nothing to do.
    Settled,
    /// A declared child route reached this coordinate and what it left stands.
    ///
    /// The commit has no plan to apply here -- see `CoordinateParticipationV3`
    /// -- so the only thing writing would accomplish is undoing the child.
    ChildPoststate,
    /// Lamports moved and nothing in this execution declared that they would.
    Unexplained,
}

/// The commit's lamport authority over ONE coordinate, as a pure decision.
///
/// Separated from the account walk so all four arms can be driven directly. The
/// arm that matters is `ChildPoststate`: the registered creation route is the
/// first in the protocol whose child CPI CREATES AND FUNDS a frame account --
/// Custody's replay and its vault -- and until this existed the commit wrote the
/// observed-vacant zero back over the child's rent and then refused its own
/// postcondition, `require_committed_accounts_persist_v3`, on the account it had
/// just emptied. Measured on real ELFs 2026-09-01: coordinate 20, 0 lamports
/// against 288 bytes needing 2,895,360, `Commit` 0x4005 at 1,205,519 CU.
///
/// `Unexplained` is the guard that used to be accidental. The old walk wrote the
/// prestate back over ANY unexplained movement, which the runtime then rejected
/// as an unbalanced instruction -- a real refusal, but one that named nothing and
/// pointed nowhere. It is stated here instead.
pub(super) const fn committed_lamports_v3(
    planned: u64,
    observed: u64,
    participation: CoordinateParticipationV3,
) -> CommittedLamportsV3 {
    if participation.locally_mutated() {
        CommittedLamportsV3::Apply
    } else if observed == planned {
        CommittedLamportsV3::Settled
    } else if participation.child_reached() {
        CommittedLamportsV3::ChildPoststate
    } else {
        CommittedLamportsV3::Unexplained
    }
}

pub(super) fn record_child_reach_and_require_disjoint_from_local(
    invocation: dclutch_vm::effect::v3::ResolvedInvocationV3,
    aliases: &[usize],
    participation: &mut [CoordinateParticipationV3],
    allowed_local_overlap: AllowedLocalOverlapV3,
) -> Result<(), ProgramError> {
    let mut refuse_window = |start: usize, end: usize| -> Result<(), ProgramError> {
        let mut coordinate = start;
        while coordinate < end {
            let representative = *aliases.get(coordinate).ok_or(TradingSbfError::Content)?;
            let slot = participation
                .get_mut(representative)
                .ok_or(TradingSbfError::Content)?;
            if slot.locally_mutated() && !allowed_local_overlap.permits(representative) {
                return Err(TradingSbfError::Content.into());
            }
            // Marked for EVERY window this walk admits, the permitted overlaps
            // included: the mark says a declared child route reaches the
            // coordinate, which is true whether or not the local effect also
            // touches it. The commit reads it beside `locally_mutated` and the
            // local plan wins when both are set, so a permitted overlap keeps
            // exactly the authority it had.
            slot.mark_child_reach();
            coordinate = coordinate.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        Ok(())
    };
    let fixed_start = usize::from(invocation.fixed_account_start);
    let fixed_end = fixed_start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    refuse_window(fixed_start, fixed_end)?;
    let item_count = usize::from(invocation.item_account_count);
    let stride = usize::from(invocation.item_account_stride);
    let mut item = 0_u32;
    while item < invocation.repeated_item_count {
        let start = invocation
            .item_account_start
            .checked_add(
                usize::try_from(item)
                    .map_err(|_| TradingSbfError::Content)?
                    .checked_mul(stride)
                    .ok_or(TradingSbfError::Content)?,
            )
            .ok_or(TradingSbfError::Content)?;
        let end = start
            .checked_add(item_count)
            .ok_or(TradingSbfError::Content)?;
        refuse_window(start, end)?;
        item = item.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(())
}

/// The complete closed set of intentional child/local observation overlaps.
///
/// This is deliberately not a list: each variant names one protocol proof and
/// fixes the only representatives that proof may admit. A third representative
/// cannot be appended by a caller or by a future descriptor extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AllowedLocalOverlapV3 {
    None,
    FractionalRoot(usize),
    SeriesExpiryReplay { root: usize, ticket: usize },
}

impl AllowedLocalOverlapV3 {
    const fn permits(self, representative: usize) -> bool {
        match self {
            Self::None => false,
            Self::FractionalRoot(root) => representative == root,
            Self::SeriesExpiryReplay { root, ticket } => {
                representative == root || representative == ticket
            }
        }
    }
}

/// Select the one local/child overlap Fractional requires.
///
/// Claims authenticates and signs with the Trading-owned root, but only
/// Trading may revise that root. The exact Fractional external-Once route may
/// therefore alias its action-selected child-root coordinate to logical root
/// zero while the local Effect writes root state. No other child coordinate,
/// route kind, request, or representative receives this exception. The root
/// bytes are re-authenticated unchanged after the verified child receipt and
/// before the commit-last pass.
pub(super) fn fractional_local_root_overlap_v3(
    invocation: dclutch_vm::effect::v3::ResolvedInvocationV3,
    request_bank: &[u8],
    family_request: &[u8],
    aliases: &[usize],
) -> Result<Option<usize>, ProgramError> {
    use dclutch_claims::fractional::{
        FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_ROOT_V3,
        FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2, FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3,
        FRACTIONAL_TERMINAL_ROOT_V3, FractionalExposureActionV2, FractionalExposureRequestV2,
    };

    if invocation.role != FixedRole::Claims
        || invocation.kind != dclutch_vm::effect::v3::RouteKindV3::Once
        || invocation.borrowed_witness.is_some()
        || family_request.get(..8) != Some(FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2.as_slice())
    {
        return Ok(None);
    }
    let request = FractionalExposureRequestV2::decode(family_request)
        .map_err(|_| TradingSbfError::Content)?;
    let (account_count, root_coordinate) = match request.action() {
        FractionalExposureActionV2::Wrap | FractionalExposureActionV2::WholeUnwrap => (
            FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3,
            FRACTIONAL_ATOMIC_ROOT_V3,
        ),
        FractionalExposureActionV2::TerminalRedeem
        | FractionalExposureActionV2::TerminalZeroBurn => (
            FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3,
            FRACTIONAL_TERMINAL_ROOT_V3,
        ),
        _ => return Ok(None),
    };
    if usize::from(invocation.fixed_account_count) != account_count
        || invocation.item_account_count != 0
        || invocation.repeated_item_count != 0
    {
        return Ok(None);
    }
    let request_end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    if request_bank.get(invocation.request_offset..request_end) != Some(family_request) {
        return Ok(None);
    }
    let logical_root = usize::from(invocation.fixed_account_start)
        .checked_add(root_coordinate)
        .ok_or(TradingSbfError::Content)?;
    if aliases.get(logical_root).copied() != Some(0) {
        return Ok(None);
    }
    Ok(Some(0))
}

/// Refuse a Fractional child that changed Trading's sole mutable root before
/// the receipt-gated local commit.
pub(super) fn verify_fractional_root_unchanged_after_children_v3(
    prepared: &PreparedHotCommitV3<'_, '_, '_, '_>,
) -> Result<(), ProgramError> {
    let root = prepared
        .frame
        .root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Commit)?;
    require_fractional_root_prestate_v3(prepared.family_request, &root, prepared.root_prestate)
}

pub(super) fn require_fractional_root_prestate_v3(
    family_request: &[u8],
    root: &[u8],
    expected_prestate: [u8; 32],
) -> Result<(), ProgramError> {
    if family_request.get(..8)
        == Some(dclutch_claims::fractional::FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2.as_slice())
        && dclutch_sha256_adapter::digest(root) != expected_prestate
    {
        return Err(TradingSbfError::Commit.into());
    }
    Ok(())
}

pub(super) fn require_no_common_projection_child_accounts_v3(
    invocation: dclutch_vm::effect::v3::ResolvedInvocationV3,
) -> Result<(), ProgramError> {
    const RESERVED_END: usize = 5;
    let fixed_start = usize::from(invocation.fixed_account_start);
    let fixed_count = usize::from(invocation.fixed_account_count);
    let fixed_end = fixed_start
        .checked_add(fixed_count)
        .ok_or(TradingSbfError::Content)?;
    if fixed_count != 0 && fixed_start < RESERVED_END && fixed_end > 0 {
        return Err(TradingSbfError::Content.into());
    }
    let item_count = usize::from(invocation.item_account_count);
    let stride = usize::from(invocation.item_account_stride);
    let mut item = 0_u32;
    while item < invocation.repeated_item_count {
        let start = invocation
            .item_account_start
            .checked_add(
                usize::try_from(item)
                    .map_err(|_| TradingSbfError::Content)?
                    .checked_mul(stride)
                    .ok_or(TradingSbfError::Content)?,
            )
            .ok_or(TradingSbfError::Content)?;
        let end = start
            .checked_add(item_count)
            .ok_or(TradingSbfError::Content)?;
        if item_count != 0 && start < RESERVED_END && end > 0 {
            return Err(TradingSbfError::Content.into());
        }
        item = item.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(())
}

#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family",
    feature = "outer-only"
))]
/// Count occurrences of the child program in one invocation frame, resolving
/// aliases the way every other check in this walk already does.
///
/// AN ALIAS IS NOT A SECOND OCCURRENCE. A coordinate whose representative is
/// another coordinate carries no privilege of its own -- `authenticate` reads
/// `representative_privileges` for it (`v2.rs:2360-2369`), and `cc228cdd` made
/// a nonzero privilege on one a refusal -- so counting its key as a second
/// appearance of the child program contradicts the frame's own profile.
///
/// This was the wall behind the Structured composition wall, measured on real
/// ELFs on 2026-09-01: the Claims representation wire fills an INACTIVE slot
/// with the Claims program id, and `IssueStructured`/`UnwrapStructured` are the
/// only actions that leave one inactive -- `RepresentationRequestV2::validate`
/// requires their actor-position revision ABSENT (`request.rs:494-500`). So the
/// program appeared at coordinate 19 and again at the placeholder 28, which the
/// operator's own AccountProfile already declares an `AuthenticatedRouteAlias`
/// of 19, and the preflight refused `2 != 1`. Every selected-outcome action
/// carries a live position there and counts one, which is why no route had ever
/// reached it.
///
/// The refusal is not weakened: two DISTINCT representative coordinates both
/// carrying the child program still refuse.
pub(crate) fn invocation_accounts_contain_program(
    invocation: dclutch_vm::effect::v3::ResolvedInvocationV3,
    accounts: DowngradedEffectAccountsV3<'_, '_, '_>,
    aliases: &[usize],
    program: &Pubkey,
) -> Result<usize, ProgramError> {
    let count_window = |start: usize, end: usize| -> Result<usize, ProgramError> {
        let mut total = 0_usize;
        let mut coordinate = start;
        while coordinate < end {
            if *aliases.get(coordinate).ok_or(TradingSbfError::Content)? == coordinate {
                total = total
                    .checked_add(accounts.count_program_in_window(coordinate, 1, program)?)
                    .ok_or(TradingSbfError::Content)?;
            }
            coordinate = coordinate.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        Ok(total)
    };
    let fixed_start = usize::from(invocation.fixed_account_start);
    let fixed_end = fixed_start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    let mut count = count_window(fixed_start, fixed_end)?;
    let item_count = usize::from(invocation.item_account_count);
    let stride = usize::from(invocation.item_account_stride);
    let mut item = 0_u32;
    while item < invocation.repeated_item_count {
        let start = invocation
            .item_account_start
            .checked_add(
                usize::try_from(item)
                    .map_err(|_| TradingSbfError::Content)?
                    .checked_mul(stride)
                    .ok_or(TradingSbfError::Content)?,
            )
            .ok_or(TradingSbfError::Content)?;
        let end = start
            .checked_add(item_count)
            .ok_or(TradingSbfError::Content)?;
        count = count
            .checked_add(count_window(start, end)?)
            .ok_or(TradingSbfError::Content)?;
        item = item.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(count)
}

/// Which child roles a walk will actually invoke.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[derive(Clone, Copy)]
struct RequiredRolesV3 {
    claims: bool,
    custody: bool,
    resolution: bool,
}

/// The physical accounts carrying each child role this walk will invoke.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
struct SelectedRoleProgramsV3<'info> {
    claims: Option<AccountInfo<'info>>,
    custody: Option<AccountInfo<'info>>,
    #[cfg(feature = "families")]
    resolution: Option<AccountInfo<'info>>,
}

/// Resolve every child role's carrier from ONE decode of the activation cache.
///
/// `ActivatedExecutionReleaseSetViewV1::decode` is not cheap: it validates the
/// whole projection, which decodes all five roles once and then decodes two
/// more per pair for the ten aliasing comparisons. Asking it per role cost
/// **58,035 CU for the three roles a Direct walk resolves** -- measured at the
/// `pf-role-programs` checkpoint against a diagnostically lifted heap -- and
/// the walk is made twice, in preflight and again in execution, so one
/// account whose bytes cannot change between them was decoded six times.
/// One decode per walk measures **37,317**: 20,718 CU per walk, 41,436 across
/// the pair. The remainder is the carrier scan, which is per role by nature.
///
/// This is deliberately ONE out-of-line function rather than a value the walks
/// hold. `execute_child_routes_v3` is against the SBPF v0 4,096-byte static
/// frame bound: holding the three decoded addresses as a walk-local made
/// `cargo build-sbf` report four frame-overwrite diagnostics on it. Here the
/// addresses live in this function's frame and only the three `AccountInfo`
/// handles the walk already held cross back.
///
/// The release-set identity is checked once, where it was checked per role
/// before: every address returned is consumed for the release set the caller
/// states, so one check covers all of them.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[inline(never)]
fn selected_role_programs_v3<'info>(
    frame: HotFrameV3<'_, 'info>,
    accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    aliases: &[usize],
    release_set: [u8; 32],
    child_programs: Option<AuthenticatedChildProgramsV3>,
    required: RequiredRolesV3,
) -> Result<SelectedRoleProgramsV3<'info>, ProgramError> {
    let (claims, custody, resolution) =
        if let Some(children) = child_programs.filter(|_| !required.resolution) {
            (
                required.claims.then_some(children.claims),
                required.custody.then_some(children.custody),
                None,
            )
        } else {
            // AUTHENTICATE, THEN READ -- here, not upstream. This branch selects
            // the Claims/Custody/Resolution programs the child walk will invoke,
            // and it used to decode `frame.activation_cache` with no owner check,
            // no derivation and no delegation to the blessed authenticator,
            // resting on the argument that some caller had already done it. The
            // argument may even have held on the accelerator path, where
            // `child_programs` is `Some` because
            // `authenticate_accelerator_activation_v4` produced it -- but this
            // branch is exactly the path where that is NOT true: it runs when
            // there is no accelerator, or when a Resolution role forces it.
            //
            // A cached role is where authority most easily goes unexplained,
            // because the authentication happened once in another program and
            // everything downstream reads the result. So the read carries its own
            // proof: owner is the Registry, not signer, not writable, not
            // executable, exact width, and the address reproduced from
            // `[ACTIVATION_PDA_DOMAIN_V1, release_set, bump]` under the Registry.
            // The witness is dropped because the value is the refusal, not the
            // token.
            let _cache_authenticated = require_activation_cache_account_v3(frame, release_set)?;
            let cache = frame
                .activation_cache
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Release)?;
            let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache)
                .map_err(|_| TradingSbfError::Release)?;
            if activated
                .execution_release_set_id()
                .map_err(|_| TradingSbfError::Release)?
                .to_bytes()
                != release_set
            {
                return Err(TradingSbfError::Release.into());
            }
            let program = |role| -> Result<[u8; 32], ProgramError> {
                Ok(activated
                    .role(role)
                    .map_err(|_| TradingSbfError::Release)?
                    .release()
                    .program()
                    .to_bytes())
            };
            let programs = (
                required
                    .claims
                    .then(|| program(ExecutionRoleV1::Claims))
                    .transpose()?,
                required
                    .custody
                    .then(|| program(ExecutionRoleV1::Custody))
                    .transpose()?,
                required
                    .resolution
                    .then(|| program(ExecutionRoleV1::Resolution))
                    .transpose()?,
            );
            drop(cache);
            programs
        };
    #[cfg(not(feature = "families"))]
    let _ = resolution;
    Ok(SelectedRoleProgramsV3 {
        claims: claims
            .map(|expected| resolve_role_carrier_v3(accounts, aliases, expected))
            .transpose()?,
        custody: custody
            .map(|expected| resolve_role_carrier_v3(accounts, aliases, expected))
            .transpose()?,
        #[cfg(feature = "families")]
        resolution: resolution
            .map(|expected| resolve_role_carrier_v3(accounts, aliases, expected))
            .transpose()?,
    })
}

/// The one physical account in the downgraded logical vector carrying a role's
/// activated program.
///
/// A role's callee must resolve to exactly one PHYSICAL account, not to exactly
/// one logical coordinate. `downgraded_effect_accounts_v3` pushes one entry per
/// logical coordinate, aliases included, and an `AuthenticatedRouteAlias` is
/// downgraded with its representative's privileges rather than skipped -- so a
/// program that several child frames legitimately name appears once per frame
/// that names it. Three clones of one `AccountInfo` are one account named three
/// times, and resolving it is unambiguous. The uniqueness test used to count
/// logical coordinates where it meant physical accounts, which refused every
/// topology whose callee is a member of a child frame: Series' three carriers
/// of the Custody program, and Dealer's and General's new ones. Two DISTINCT
/// physical accounts carrying the role's key stays refused, which is the case
/// the test was written for.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
fn resolve_role_carrier_v3<'info>(
    accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    aliases: &[usize],
    expected: [u8; 32],
) -> Result<AccountInfo<'info>, ProgramError> {
    // `accounts` is the downgraded LOGICAL vector and `aliases` is the
    // per-logical-coordinate representative table built at the same registers.
    // They are the same length by construction; the resolver refuses rather
    // than assuming it, because an `aliases` longer than `accounts` would read
    // as a silent short scan rather than as an error.
    resolve_carrier_by_representative_v3(accounts.len(), aliases, expected, |coordinate| {
        accounts.view(coordinate)
    })
}

/// The dedup itself, over any per-coordinate child view.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
pub(super) fn resolve_carrier_by_representative_v3<'info>(
    len: usize,
    aliases: &[usize],
    expected: [u8; 32],
    view: impl Fn(usize) -> Result<AccountInfo<'info>, ProgramError>,
) -> Result<AccountInfo<'info>, ProgramError> {
    if len != aliases.len() {
        hot_cu_role_carrier!(0, len as u64, aliases.len() as u64, 0);
        return Err(TradingSbfError::Release.into());
    }
    let mut found: Option<(usize, AccountInfo<'info>)> = None;
    let mut coordinate = 0_usize;
    while coordinate < len {
        let account = view(coordinate)?;
        if account.key.to_bytes() != expected {
            coordinate = coordinate.checked_add(1).ok_or(TradingSbfError::Content)?;
            continue;
        }
        // Per-account, and BEFORE the dedup: a carrier that arrived writable or
        // signing is refused on its own terms, never absorbed into a
        // representative that happens to be clean.
        if !account.executable || account.is_signer || account.is_writable {
            hot_cu_role_carrier!(
                1,
                coordinate as u64,
                u64::from(account.executable),
                u64::from(account.is_signer) | (u64::from(account.is_writable) << 1)
            );
            return Err(TradingSbfError::Release.into());
        }
        let representative = representative_v3(coordinate, aliases)?;
        match found {
            Some((seen, _)) if seen != representative => {
                hot_cu_role_carrier!(2, coordinate as u64, seen as u64, representative as u64);
                return Err(TradingSbfError::Release.into());
            }
            Some(_) => {}
            None => found = Some((representative, account)),
        }
        coordinate = coordinate.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    found.map(|(_, account)| account).ok_or_else(|| {
        // The role's activated program is not in the child frame AT ALL. The
        // first four bytes of the key it looked for are enough to say which
        // program went missing without logging thirty-two.
        hot_cu_role_carrier!(
            3,
            u64::from(u32::from_be_bytes([
                expected[0],
                expected[1],
                expected[2],
                expected[3]
            ])),
            len as u64,
            0
        );
        TradingSbfError::Release.into()
    })
}

/// Execute one Claims route and return ONLY the digest of what it produced.
///
/// `ClaimsRouteReceiptV3` is 520 bytes -- it is the union of eight receipt
/// bodies -- and `claims_receipt_digest_v3` re-materialises the selected one as
/// a byte buffer to hash it. Both of those belong to a frame that is not
/// `execute_child_routes_v3`'s: that walk is against the SBPF v0 4,096-byte
/// static frame bound, and holding the receipt across the two calls put the 520
/// bytes, the second 520-byte copy the call needed, and the eight `to_bytes`
/// temporaries the digest inlines on the walk at once. Measured on the dealer
/// accelerator link, where the walk is not the caller of any Resolution route
/// and the inliner therefore has room to take all of it: 5,184 bytes of frame
/// and 82 backend frame-overwrite diagnostics before, 2,752 and zero after.
/// Trading at default features was never over the bound and was never told: it
/// was at 3,712 of 4,096 on the same function, and is at 2,880 here.
///
/// This is the callee-owned-frame discipline the walk already applies to the
/// child-walk resolution and to the CPI buffer set, applied to the one arm that
/// returns a receipt where the others return a digest. Nothing is computed
/// anywhere else, in any other order, or on any other value: the walk asked for
/// a digest and now says so.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn execute_claims_route_digest_v3<'info>(
    program_id: &Pubkey,
    successor_account_count: usize,
    composition: ClaimsCompositionV3<'_>,
    route_index: u16,
    invocation_index: u32,
    invocation: dclutch_vm::effect::v3::ResolvedInvocationV3,
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    aliases: &[usize],
    request_bank: &[u8],
    borrowed_ranges: BorrowedRouteRangesV4<'_, '_, '_>,
    prior_receipt: Option<&[u8]>,
    buffers: &mut ChildInvocationBuffersV3<'info>,
    claims_program: &AccountInfo<'info>,
    sparse_post_resource_verification: SparsePostResourceVerificationV3,
    hint: PreflightedCallerBumpV4,
) -> Result<[u8; 32], ProgramError> {
    claims_receipt_digest_v3(execute_claims_route_v3(
        program_id,
        successor_account_count,
        composition,
        route_index,
        invocation_index,
        invocation,
        effect_accounts,
        aliases,
        request_bank,
        borrowed_ranges,
        prior_receipt,
        buffers,
        claims_program,
        sparse_post_resource_verification,
        hint,
    )?)
}

/// Hash the receipt body one Claims route produced.
///
/// Out of line for the same reason its caller is: the match materialises the
/// selected body as bytes, and the eight arms' temporaries land on whichever
/// frame inlines it.
#[inline(never)]
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
fn claims_receipt_digest_v3(receipt: ClaimsRouteReceiptV3) -> Result<[u8; 32], ProgramError> {
    let bytes = match receipt {
        ClaimsRouteReceiptV3::Admit(value) => value
            .to_receipt_bytes()
            .map(Vec::from)
            .map_err(|_| TradingSbfError::Transition)?,
        ClaimsRouteReceiptV3::Affine(value) => Vec::from(value.to_bytes()),
        ClaimsRouteReceiptV3::SignedDelta(value) => Vec::from(value.to_bytes()),
        ClaimsRouteReceiptV3::SparseNativeTransfer(value) => Vec::from(value.to_bytes()),
        ClaimsRouteReceiptV3::Founding(value) => Vec::from(value.to_bytes()),
        ClaimsRouteReceiptV3::RationalLifecycle(value) => value
            .to_bytes()
            .map(Vec::from)
            .map_err(|_| TradingSbfError::Transition)?,
        ClaimsRouteReceiptV3::RationalRepresentation(value) => value
            .to_bytes()
            .map(Vec::from)
            .map_err(|_| TradingSbfError::Transition)?,
        ClaimsRouteReceiptV3::FractionalAtomic(value) => Vec::from(value.to_bytes()),
        ClaimsRouteReceiptV3::FractionalTerminalAtomic(value) => Vec::from(value.to_bytes()),
        ClaimsRouteReceiptV3::FractionalRetirementCoordinate(value) => Vec::from(value.to_bytes()),
        ClaimsRouteReceiptV3::FractionalClaimCheckCompaction(value) => value
            .to_bytes()
            .map(Vec::from)
            .map_err(|_| TradingSbfError::Transition)?,
        ClaimsRouteReceiptV3::Close(value) => value
            .to_bytes()
            .map(Vec::from)
            .map_err(|_| TradingSbfError::Transition)?,
    };
    Ok(hashv(&[CHILD_EXECUTION_DIGEST_DOMAIN_V3, &bytes]).to_bytes())
}

#[derive(Clone, Copy)]
pub(super) enum RequestProfileKindV3<'a> {
    Unsigned(RequestProfileV1<'a>),
    Signed(RequestProfileV2<'a>),
    Borrowed(RequestProfileV3<'a>),
    RepeatedRows(RequestProfileV4<'a>),
}

impl<'a> RequestProfileKindV3<'a> {
    /// Borrow the exact canonical record body this profile was decoded from.
    pub(super) const fn bytes(self) -> &'a [u8] {
        match self {
            Self::Unsigned(profile) => profile.bytes(),
            Self::Signed(profile) => profile.bytes(),
            Self::Borrowed(profile) => profile.bytes(),
            Self::RepeatedRows(profile) => profile.bytes(),
        }
    }

    pub(super) const fn v1(self) -> RequestProfileV1<'a> {
        match self {
            Self::Unsigned(profile) => profile,
            Self::Signed(profile) => profile.request_profile(),
            Self::Borrowed(profile) => profile.request_profile(),
            Self::RepeatedRows(profile) => profile.request_profile(),
        }
    }

    pub(super) fn writes_register(self, target: ProjectionTargetV1) -> Result<bool, ProgramError> {
        match self {
            Self::RepeatedRows(profile) => profile
                .writes_register(target)
                .map_err(|_| TradingSbfError::Content.into()),
            Self::Unsigned(_) | Self::Signed(_) | Self::Borrowed(_) => self
                .v1()
                .writes_register(target)
                .map_err(|_| TradingSbfError::Content.into()),
        }
    }

    pub(super) fn writes_any_register(self, targets: &[ProjectionTargetV1]) -> Result<bool, ProgramError> {
        match self {
            Self::RepeatedRows(profile) => profile
                .writes_any_register(targets)
                .map_err(|_| TradingSbfError::Content.into()),
            Self::Unsigned(_) | Self::Signed(_) | Self::Borrowed(_) => self
                .v1()
                .writes_any_register(targets)
                .map_err(|_| TradingSbfError::Content.into()),
        }
    }

    pub(super) fn project_atomic(
        self,
        tail_count: u32,
        family_request: &'a [u8],
        registers: ProjectionRegistersV1<'_>,
    ) -> Result<(), ProgramError> {
        match self {
            Self::Unsigned(profile) => {
                project_request_atomic(profile, tail_count, family_request, registers)
                    .map_err(|_| TradingSbfError::Content.into())
            }
            Self::Signed(profile) => project_request_atomic(
                profile.request_profile(),
                tail_count,
                family_request,
                registers,
            )
            .map_err(|_| TradingSbfError::Content.into()),
            Self::Borrowed(profile) => profile
                .project_prefix_atomic(tail_count, family_request, registers)
                .map_err(|_| TradingSbfError::Content.into()),
            Self::RepeatedRows(profile) => {
                let mut candidate_scalars = vec![0_u64; registers.output_scalars.len()];
                let mut candidate_identities = vec![[0_u8; 32]; registers.output_identities.len()];
                profile
                    .project_atomic(
                        family_request,
                        ProjectionRegistersV4 {
                            input_scalars: registers.input_scalars,
                            input_identities: registers.input_identities,
                            scratch_scalars: registers.scratch_scalars,
                            scratch_identities: registers.scratch_identities,
                            candidate_scalars: &mut candidate_scalars,
                            candidate_identities: &mut candidate_identities,
                            output_scalars: registers.output_scalars,
                            output_identities: registers.output_identities,
                        },
                    )
                    .map_err(|_| TradingSbfError::Content.into())
            }
        }
    }

    pub(super) fn require_request_shape(
        self,
        tail_count: u32,
        family_request: &'a [u8],
    ) -> Result<(), ProgramError> {
        match self {
            Self::Borrowed(profile) => profile
                .split_request(tail_count, family_request)
                .map(|_| ())
                .map_err(|_| TradingSbfError::Content.into()),
            Self::RepeatedRows(profile) => {
                if profile
                    .request_bytes()
                    .map_err(|_| TradingSbfError::Content)?
                    == family_request.len()
                {
                    Ok(())
                } else {
                    Err(TradingSbfError::Content.into())
                }
            }
            Self::Unsigned(_) | Self::Signed(_) => {
                if self
                    .v1()
                    .request_bytes(tail_count)
                    .map_err(|_| TradingSbfError::Content)?
                    == family_request.len()
                {
                    Ok(())
                } else {
                    Err(TradingSbfError::Content.into())
                }
            }
        }
    }
}

/// Select and construct the request profile from a Trading-sealed record.
///
/// The live dispatcher re-hashes the record to produce its `authenticated`
/// argument; the sealed one does not, because `borrow_sealed_record` has
/// already required `hash(bytes)` to be exactly the identity the authenticated
/// descriptor names. The schema selection is unchanged and still comes from
/// the descriptor.
pub(super) fn decode_sealed_request_profile<'a>(
    descriptor: CapabilityProgramV4,
    bytes: &'a [u8],
    sealed: SealedArtifactV1<'_>,
) -> Result<RequestProfileKindV3<'a>, ProgramError> {
    let schema = descriptor.request_profile().schema().to_bytes();
    if schema == REQUEST_PROFILE_SCHEMA_ID_V1 {
        RequestProfileV1::from_sealed(bytes, sealed)
            .map(RequestProfileKindV3::Unsigned)
            .map_err(|_| TradingSbfError::Content.into())
    } else if schema == REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID {
        RequestProfileV2::from_sealed(bytes, sealed)
            .map(RequestProfileKindV3::Signed)
            .map_err(|_| TradingSbfError::Content.into())
    } else if schema == REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID {
        RequestProfileV3::from_sealed(bytes, sealed)
            .map(RequestProfileKindV3::Borrowed)
            .map_err(|_| TradingSbfError::Content.into())
    } else if schema == REQUEST_PROFILE_V4_SCHEMA_RELEASE_ID {
        RequestProfileV4::from_sealed(bytes, sealed)
            .map(RequestProfileKindV3::RepeatedRows)
            .map_err(|_| TradingSbfError::Content.into())
    } else {
        Err(TradingSbfError::UnsupportedContent.into())
    }
}

#[allow(dead_code)]
pub(super) fn decode_request_profile<'a>(
    descriptor: CapabilityProgramV4,
    bytes: &'a [u8],
) -> Result<RequestProfileKindV3<'a>, ProgramError> {
    let selected = descriptor.request_profile().program().to_bytes();
    let authenticated = hash(bytes).to_bytes();
    if descriptor.request_profile().schema().to_bytes() == REQUEST_PROFILE_SCHEMA_ID_V1 {
        RequestProfileV1::decode_selected(selected, authenticated, bytes)
            .map(RequestProfileKindV3::Unsigned)
            .map_err(|_| TradingSbfError::Content.into())
    } else if descriptor.request_profile().schema().to_bytes()
        == REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID
    {
        RequestProfileV2::decode_selected(selected, authenticated, bytes)
            .map(RequestProfileKindV3::Signed)
            .map_err(|_| TradingSbfError::Content.into())
    } else if descriptor.request_profile().schema().to_bytes()
        == REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID
    {
        RequestProfileV3::decode_selected(selected, authenticated, bytes)
            .map(RequestProfileKindV3::Borrowed)
            .map_err(|_| TradingSbfError::Content.into())
    } else if descriptor.request_profile().schema().to_bytes()
        == REQUEST_PROFILE_V4_SCHEMA_RELEASE_ID
    {
        RequestProfileV4::decode_selected(selected, authenticated, bytes)
            .map(RequestProfileKindV3::RepeatedRows)
            .map_err(|_| TradingSbfError::Content.into())
    } else {
        Err(TradingSbfError::UnsupportedContent.into())
    }
}

/// Construct the selected effect program from a Trading-sealed record.
#[inline(never)]
pub(super) fn decode_sealed_effect_v4<'a>(
    schema: [u8; 32],
    bytes: &'a [u8],
    sealed: SealedArtifactV1<'_>,
) -> Result<SelectedEffectProgramV4<'a>, ProgramError> {
    let (successor, funding) = if schema == EFFECT_SCHEMA_ID_V4 {
        (
            EffectProgramV4::from_sealed(bytes, sealed).map_err(|_| TradingSbfError::Content)?,
            None,
        )
    } else if schema == EFFECT_SCHEMA_ID_V5 {
        let funding =
            EffectProgramV5::from_sealed(bytes, sealed).map_err(|_| TradingSbfError::Content)?;
        (funding.base(), Some(funding))
    } else {
        return Err(TradingSbfError::UnsupportedContent.into());
    };
    // Profile13 and the EffectV4 kernel jointly own selected account spans and
    // borrowed family-request ranges. The sealed Effect is the range authority;
    // runtime coverage is checked after request projection resolves its scalar
    // coordinates and before any lifecycle or child mutation.
    Ok(SelectedEffectProgramV4 {
        base: successor.base(),
        successor,
        funding,
    })
}

#[allow(dead_code)]
#[inline(never)]
pub(super) fn decode_selected_effect_v4<'a>(
    schema: [u8; 32],
    bytes: &'a [u8],
) -> Result<SelectedEffectProgramV4<'a>, ProgramError> {
    let (successor, funding) = if schema == EFFECT_SCHEMA_ID_V4 {
        (
            EffectProgramV4::decode(bytes).map_err(|_| TradingSbfError::Content)?,
            None,
        )
    } else if schema == EFFECT_SCHEMA_ID_V5 {
        let funding = EffectProgramV5::decode(bytes).map_err(|_| TradingSbfError::Content)?;
        (funding.base(), Some(funding))
    } else {
        return Err(TradingSbfError::UnsupportedContent.into());
    };
    // The unsealed test/migration path admits the identical successor grammar.
    // Runtime coverage and child append are shared with the sealed path.
    Ok(SelectedEffectProgramV4 {
        base: successor.base(),
        successor,
        funding,
    })
}
