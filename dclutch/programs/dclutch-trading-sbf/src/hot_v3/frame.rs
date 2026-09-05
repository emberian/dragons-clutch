//! The Hot frame and what it authenticates before any family runs: the Market,
//! the immutable root and its roles, the Product runtime, the sealed strategy.

use super::*;

pub(super) struct AuthenticatedRootV3 {
    pub(super) context: TradingFamilyContextV1,
    pub(super) immutable_header: [u8; CAPABILITY_ROOT_HEADER_BYTES_V1],
    pub(super) trading_semantic_release: [u8; 32],
    pub(super) child_programs: Option<AuthenticatedChildProgramsV3>,
}

#[derive(Clone, Copy)]
pub(super) struct AuthenticatedChildProgramsV3 {
    pub(super) claims: [u8; 32],
    pub(super) custody: [u8; 32],
}

#[inline(never)]
pub(super) fn parse_hot_frame_boxed_v3<'accounts, 'info>(
    program_id: &Pubkey,
    accounts: &'accounts [AccountInfo<'info>],
    permits_fixed_market_union: bool,
) -> Result<Box<HotFrameV3<'accounts, 'info>>, ProgramError> {
    HotFrameV3::parse(program_id, accounts, permits_fixed_market_union).map(Box::new)
}

#[inline(never)]
pub(super) fn authenticate_market_boxed_v3<'accounts, 'info>(
    frame: &HotFrameV3<'accounts, 'info>,
    envelope: HotExecutionEnvelopeV3,
) -> Result<Box<CoreState>, ProgramError> {
    authenticate_market(*frame, envelope).map(Box::new)
}

#[inline(never)]
pub(super) fn authenticate_root_boxed_v3<'accounts, 'info>(
    program_id: &Pubkey,
    frame: &HotFrameV3<'accounts, 'info>,
    envelope: HotExecutionEnvelopeV3,
    market: &CoreState,
    role_authentication: HotRoleAuthenticationV3,
) -> Result<Box<AuthenticatedRootV3>, ProgramError> {
    authenticate_root_against_market_boxed_v3(
        program_id,
        frame,
        envelope,
        market.identity.market_id.to_bytes(),
        role_authentication,
    )
}

#[inline(never)]
fn authenticate_root_against_market_boxed_v3<'accounts, 'info>(
    program_id: &Pubkey,
    frame: &HotFrameV3<'accounts, 'info>,
    envelope: HotExecutionEnvelopeV3,
    expected_market: [u8; 32],
    role_authentication: HotRoleAuthenticationV3,
) -> Result<Box<AuthenticatedRootV3>, ProgramError> {
    // Both arms are ONE out-of-line call. This route runs within a few
    // thousand CU of the 1,400,000 ceiling at four outcomes, so whichever arm
    // a caller takes must not pay for the other arm's registers or frame.
    let (trading_receipt, child_programs) = match role_authentication {
        HotRoleAuthenticationV3::ReauthenticateRegistry => {
            reauthenticate_top_level_root_roles_v3(*frame, envelope)?
        }
        HotRoleAuthenticationV3::AuthenticatedContinuation => {
            authenticate_continuation_root_roles_v3(*frame, envelope)?
        }
    };
    let child_programs = Some(child_programs);
    let trading_semantic_release = trading_receipt.semantic_release_id().to_bytes();
    let root_data = frame
        .root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Root)?;
    let context = TradingFamilyContextV1::authenticate_at(
        program_id,
        frame.root.key,
        frame.root.owner,
        &root_data,
        trading_receipt,
        envelope.bump_hints().root,
    )?;
    let root_header = CapabilityRootHeaderV1::decode(
        root_data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(TradingSbfError::Root)?,
    )
    .map_err(|_| TradingSbfError::Root)?;
    if context.market() != envelope.market()
        || context.release_set().to_bytes() != envelope.release_set()
        || context.generation() != envelope.generation()
        || expected_market != envelope.market()
    {
        return Err(TradingSbfError::Root.into());
    }
    Ok(Box::new(AuthenticatedRootV3 {
        context,
        immutable_header: root_header.to_bytes(),
        trading_semantic_release,
        child_programs,
    }))
}

/// Authenticate Core and Trading for a caller who invoked Trading DIRECTLY.
///
/// What the Registry did with those 52,592 CU was decode an account this frame
/// holds and read two roles out of it. Every child role program already reads
/// the same account the same way, because under a continuation it must -- the
/// Registry sits at depth one there and the CPI is reentrancy. So the local read
/// is not a new trust shape being introduced on this arm; it is the shape the
/// other four families have run since 2026-08-27, arriving where it was merely
/// expensive rather than impossible.
///
/// The heap check stays first, before anything allocates.
#[inline(never)]
fn reauthenticate_top_level_root_roles_v3(
    frame: HotFrameV3<'_, '_>,
    envelope: HotExecutionEnvelopeV3,
) -> Result<(AuthenticatedRoleReceiptV1, AuthenticatedChildProgramsV3), ProgramError> {
    // Before the reauthentication frames, not after: this route's peak exceeds
    // the protocol default heap and this is the first thing it spends that
    // budget on. Refusing here costs a caller who forgot the grant one
    // comparison instead of a million compute units and an unnamed abort.
    crate::entrypoint_adapter::require_declared_heap_ceiling_above_default_v1()?;
    // Owner, non-executability and the one exact width BEFORE a byte is read,
    // which is the ordering `dclutch-registry-activation-auth-v1` documents and
    // the reason a stranger's account can never contribute the bump seed the
    // identity check below reproduces the address from.
    require_cache_account(frame.registry.key, frame.activation_cache)
        .map_err(TradingSbfError::from)?;
    authenticate_top_level_root_roles_from_cache_v3(frame, envelope)
}

/// One borrow, one decode, four roles.
///
/// Split out so the borrow's `Ref` and the view that lives inside it have a
/// scope, and so the continuation arm -- which does not call this -- carries
/// none of its frame.
///
/// # The conjunction, and where each half of it comes from
///
/// The two CPIs this replaces ran `process_reauthenticate`
/// (`programs/dclutch-registry-sbf/src/lib.rs`), which is: the three-account
/// read-only frame, a hostile `decode` of the cache, `authenticate_cache_identity`
/// (Registry ownership, non-executability, exact width, address reproduced from
/// the body's carried bump), and then `authenticate_activated_role_in_cache_v1`
/// for the role. Everything below the identity check is a function this program
/// now calls DIRECTLY, and it is the same function object the Registry calls --
/// so the two readers cannot drift, which is the property decision 0017 §3 named
/// as better than the CPI had.
///
/// The identity check is the crate's too, and it is STRICTER here than in the
/// CPI. `authenticate_cache_identity` derived the cache address from the
/// address the CACHE ITSELF names, so a perfectly valid cache belonging to
/// another Market passed it; what refused that was the caller's after-the-fact
/// `receipt.execution_release_set_id() == release_set` comparison, which
/// somebody had to remember to write. `authenticate_activation_cache_identity_v1`
/// derives it from the release set THIS Market selected, so the wrong-generation
/// cache refuses at its own address before a role is decoded.
///
/// # Why three decodes became one
///
/// `ActivatedExecutionReleaseSetViewV1::decode` validates the complete five-role
/// projection and all ten aliasing pairs -- twenty-five `decode_role` calls --
/// and this route ran it three times: once in each Registry CPI and once more
/// for the children. Seventy-five role decodes of one immutable 1,288-byte
/// account, for one answer. The account cannot change between them: it is
/// Registry-owned, this frame holds it read-only, and its content is fixed for
/// the life of its release set.
#[inline(never)]
pub(super) fn authenticate_top_level_root_roles_from_cache_v3(
    frame: HotFrameV3<'_, '_>,
    envelope: HotExecutionEnvelopeV3,
) -> Result<(AuthenticatedRoleReceiptV1, AuthenticatedChildProgramsV3), ProgramError> {
    let data = frame
        .activation_cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated =
        ActivatedExecutionReleaseSetViewV1::decode(&data).map_err(|_| TradingSbfError::Release)?;
    authenticate_activation_cache_identity_v1(
        frame.registry,
        frame.activation_cache,
        &envelope.release_set(),
        activated,
    )
    .map_err(TradingSbfError::from)?;
    let core_receipt = authenticate_activated_role_in_frame_v1(
        frame.activation_cache,
        activated,
        ExecutionRoleV1::Core,
        frame.core_program,
        frame.core_programdata,
    )
    .map_err(TradingSbfError::from)?;
    // Kept although `cached_role_deployment_observation_v1` already required
    // `program.key == release.program()` before it observed anything: this is
    // the comparison the CPI arm made on the returned receipt, and dropping it
    // in the same change that removes the CPI would make the diff say something
    // it does not mean.
    if core_receipt.program().as_bytes() != &frame.core_program.key.to_bytes() {
        return Err(TradingSbfError::Release.into());
    }
    let trading_receipt = authenticate_activated_role_in_frame_v1(
        frame.activation_cache,
        activated,
        ExecutionRoleV1::Trading,
        frame.trading_program,
        frame.trading_programdata,
    )
    .map_err(TradingSbfError::from)?;
    Ok((
        trading_receipt,
        read_activated_child_programs_v3(activated)?,
    ))
}

/// The continuation arm of the same split, and it carries the same heap guard
/// as its sibling for the same reason.
///
/// The 252 bytes past the default are reached by an INFALLIBLE allocation, so
/// the run does not refuse, it ABORTS -- Trading logging "memory allocation
/// failed, out of memory" and the runtime reporting
/// `ProgramFailedToComplete`, which carries no code at all. Three hostiles in
/// `registry_hot_continuation` then asserted exact refusals against that and
/// passed on nothing, which is ledger M-38's universal donor exactly.
///
/// `HeapFrame` 0x4008 is the right code and `HeapExhausted` 0x4027 is not: the
/// enum's own doc splits them on whether a grant ARRIVED. A continuation whose
/// transaction sent no `RequestHeapFrame` never asked, so this is the "grant
/// the transaction never asked for" end, and the remedy it names is the one the
/// caller can act on -- send one. `waist::direct_registry_instructions` is what
/// sends it on the harness's canonical frame.
fn authenticate_continuation_root_roles_v3(
    frame: HotFrameV3<'_, '_>,
    envelope: HotExecutionEnvelopeV3,
) -> Result<(AuthenticatedRoleReceiptV1, AuthenticatedChildProgramsV3), ProgramError> {
    crate::entrypoint_adapter::require_declared_heap_ceiling_above_default_v1()?;
    let (trading_receipt, claims, custody) =
        authenticate_accelerator_activation_v4(frame, envelope)?;
    Ok((
        trading_receipt,
        AuthenticatedChildProgramsV3 {
            claims: claims.to_bytes(),
            custody: custody.to_bytes(),
        },
    ))
}

/// The prelude's own Product graph walk, at the bumps THIS MARKET recorded.
///
/// The founding is the only party that derives these addresses from a graph it
/// has already authenticated, and since 2026-09-03 it writes them into the
/// Market's `StateBumpsV1`. Every reader here was searching for four Registry
/// record pairs whose seeds are a PDA domain, a canonical schema id and a
/// content digest -- none of which moves with the release set, so the search
/// depth is fixed and paid on every instruction.
///
/// Reading the bank cannot refuse. A market founded before the field existed
/// carries eight zeros, `ProductRecordBumpsV3::ABSENT` is what that means, and
/// this walk searches exactly as it used to.
#[inline(never)]
pub(super) fn authenticate_product_runtime_boxed_v3<'accounts, 'info>(
    frame: &HotFrameV3<'accounts, 'info>,
    market: &CoreState,
) -> Result<Box<AuthenticatedProductRuntimeV3<'accounts, 'info>>, ProgramError> {
    authenticate_product_runtime_hinted_boxed_v3(
        frame,
        market,
        ProductRecordBumpsV3(market.bumps.product_graph.bumps()),
    )
}

/// The same walk over the same four records, at the bumps its CALLER derived.
///
/// This exists for exactly one route. The Dealer accelerator runs the Product
/// graph walk a second time, independently, on the far side of a CPI, over the
/// same four Registry records the caller authenticated a few thousand
/// instructions earlier -- and `authenticate_record` runs two
/// `find_program_address` calls per record. Measured 2026-09-03 by doubling
/// those eight searches: they are 30,172 CU of the 39,217-CU
/// `acc-product-runtime` span, identical to the digit on all nine invocations
/// of one campaign run, because their seeds are a schema and a content digest
/// and neither moves with the release set.
///
/// Nothing about the authentication moves. Each bump is fed to
/// `create_program_address` over seeds this program derives for itself and the
/// result is compared against the account the frame supplied, by the equality
/// that was always there.
#[inline(never)]
pub(super) fn authenticate_product_runtime_hinted_boxed_v3<'accounts, 'info>(
    frame: &HotFrameV3<'accounts, 'info>,
    market: &CoreState,
    hints: ProductRecordBumpsV3,
) -> Result<Box<AuthenticatedProductRuntimeV3<'accounts, 'info>>, ProgramError> {
    authenticate_product_runtime_v3_hinted(
        frame.registry.key,
        ProductContentId::new(market.identity.product_record.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        ProductRuntimeFrameV3 {
            product: ProductRecordFrameV2 {
                raw: frame.product_raw,
                staging: frame.product_staging,
            },
            result_domain: ProductRecordFrameV2 {
                raw: frame.result_domain_raw,
                staging: frame.result_domain_staging,
            },
            portfolio: ProductRecordFrameV2 {
                raw: frame.portfolio_raw,
                staging: frame.portfolio_staging,
            },
            linked_basis: ProductRecordFrameV2 {
                raw: frame.linked_basis_raw,
                staging: frame.linked_basis_staging,
            },
        },
        hints,
    )
    .map(Box::new)
    .map_err(|_| TradingSbfError::Content.into())
}

#[inline(never)]
pub(super) fn decode_capability_program_boxed_v3(
    descriptor_data: &[u8],
) -> Result<Box<CapabilityProgramV4>, ProgramError> {
    CapabilityProgramV4::decode(descriptor_data)
        .map(Box::new)
        .map_err(|_| TradingSbfError::Content.into())
}

#[inline(never)]
pub(super) fn authenticate_manifest_entry_boxed_v3(
    manifest_data: &[u8],
    context: &TradingFamilyContextV1,
) -> Result<Box<dclutch_capability_contract::CapabilityEntryV1>, ProgramError> {
    let manifest =
        CapabilityManifestV1::decode(manifest_data).map_err(|_| TradingSbfError::Content)?;
    let entry = manifest
        .entry(context.selection().entry_index())
        .map_err(|_| TradingSbfError::Content)?;
    if entry.kind_id() != context.selection().kind()
        || entry.release_id() != context.selection().capability_release()
        || entry.config_id() != context.selection().config()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(Box::new(entry))
}

#[inline(never)]
pub(super) fn authenticate_strategy_from_sealed_boxed_v3<'accounts, 'info>(
    frame: &HotFrameV3<'accounts, 'info>,
    accounts: &'accounts [AccountInfo<'info>],
    context: TradingFamilyContextV1,
    selected_program: ContentId,
    descriptor: &CapabilityProgramV4,
    strategy_extras_start: usize,
) -> Result<(Box<AuthenticatedExecutionStrategyV2>, usize), ProgramError> {
    let strategy_data = frame
        .strategy_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if strategy_data.len() != EXECUTION_STRATEGY_PROGRAM_BYTES_V2 {
        return Err(TradingSbfError::Content.into());
    }
    let preliminary_strategy =
        ExecutionStrategyProgramV2::decode(&strategy_data).map_err(|_| TradingSbfError::Content)?;
    drop(strategy_data);
    let strategy_account_count = match preliminary_strategy.disposition() {
        StrategyDispositionV2::Interpreted => INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2,
        StrategyDispositionV2::ShadowAot => SHADOW_AOT_STRATEGY_ACCOUNT_COUNT_V2,
        StrategyDispositionV2::AdmittedAot => ADMITTED_AOT_STRATEGY_ACCOUNT_COUNT_V2,
    };
    let strategy_extra_count = strategy_account_count
        .checked_sub(INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2)
        .ok_or(TradingSbfError::Content)?;
    let strategy_extras_end = strategy_extras_start
        .checked_add(strategy_extra_count)
        .ok_or(TradingSbfError::Content)?;
    let strategy_extras = accounts
        .get(strategy_extras_start..strategy_extras_end)
        .ok_or(TradingSbfError::Content)?;
    let mut strategy_accounts = Vec::with_capacity(strategy_account_count);
    strategy_accounts.extend_from_slice(&[
        frame.descriptor_raw.clone(),
        frame.descriptor_staging.clone(),
        frame.strategy_raw.clone(),
        frame.strategy_staging.clone(),
    ]);
    strategy_accounts.extend_from_slice(strategy_extras);
    let strategy = authenticate_execution_strategy_from_sealed_capability_v2(
        context,
        selected_program,
        descriptor,
        frame.registry,
        frame.rent,
        &strategy_accounts,
    )?;
    if strategy.strategy().disposition() == StrategyDispositionV2::ShadowAot
        && strategy
            .strategy()
            .transport_profile()
            .map_err(|_| TradingSbfError::Content)?
            != AcceleratorTransportProfileV2::ShadowTranscriptV3
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    // BOTH candidate-bank transports, and `UnsupportedContent` rather than
    // `Content`. This gate cost a localization on 2026-09-02: an admitted route
    // whose Strategy named the output-page pair died here at 574,606 CU with
    // `Content` 0x4003, one of 2,126 sites, and the checkpoint trail said only
    // "somewhere between root-product and artifacts-strategy-effect". The
    // Shadow gate immediately above says the same thing about its own
    // disposition and says it as `UnsupportedContent`, which is what this is:
    // the pairing decoded fine and names a transport this disposition does not
    // admit.
    if strategy.strategy().disposition() == StrategyDispositionV2::AdmittedAot
        && !matches!(
            strategy
                .strategy()
                .transport_profile()
                .map_err(|_| TradingSbfError::UnsupportedContent)?,
            AcceleratorTransportProfileV2::ChunkedBankV2
                | AcceleratorTransportProfileV2::OutputPageV3
        )
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    Ok((Box::new(strategy), strategy_extras_end))
}

pub(super) fn authenticate_descriptor_root_selection(
    descriptor: &CapabilityProgramV4,
    context: &TradingFamilyContextV1,
    entry: &dclutch_capability_contract::CapabilityEntryV1,
) -> Result<(), ProgramError> {
    // The reason is PROPAGATED, not discarded. This was
    // `validate_selection(..).is_err() || width != ..` folded into one bare
    // `Content`, and folding three distinguishable accusations into the most
    // crowded code on the route is what made one defect look like two and cost a
    // lane a full bisect. `validate_selection` already knows which of the two it
    // is; there is nothing to derive, only something to stop throwing away.
    match descriptor.validate_selection(context.selection(), *entry) {
        Ok(()) => {}
        Err(CapabilityProgramError::SelectionMismatch) => {
            return Err(TradingSbfError::DescriptorKind.into());
        }
        // Every other variant this call can return is an entry-profile
        // disagreement; naming them individually would publish codes for states
        // `validate_selection` cannot reach.
        Err(_) => return Err(TradingSbfError::DescriptorManifestEntry.into()),
    }
    // `Root` when the width cannot be decoded, `DescriptorRootWidth` when it
    // decoded and disagreed -- two different things about the same field, and
    // the first was already distinct before this change.
    if descriptor
        .root_account_bytes()
        .map_err(|_| TradingSbfError::Root)?
        != context.root_account_bytes()
    {
        return Err(TradingSbfError::DescriptorRootWidth.into());
    }
    Ok(())
}

fn authenticate_market(
    frame: HotFrameV3<'_, '_>,
    envelope: HotExecutionEnvelopeV3,
) -> Result<CoreState, ProgramError> {
    if frame.market.owner != frame.core_program.key || frame.market.data_len() != STATE_BYTES {
        return Err(TradingSbfError::Content.into());
    }
    let bytes = frame
        .market
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let state = CoreState::decode(&bytes).map_err(|_| TradingSbfError::Content)?;
    if state
        .encode()
        .map_err(|_| TradingSbfError::Content)?
        .as_slice()
        != bytes.as_ref()
        || state.identity.market_id.to_bytes() != frame.market.key.to_bytes()
        || state.identity.selected_release_set.to_bytes() != envelope.release_set()
        || state.identity.registry_program.to_bytes() != frame.registry.key.to_bytes()
        || state.identity.generation != envelope.generation()
        || envelope.market() != frame.market.key.to_bytes()
        || market_core_state_address_v2(
            state,
            frame.core_program.key,
            envelope.bump_hints().market,
        )? != *frame.market.key
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(state)
}

/// Reproduce a Market's own Core state address, or search for it.
///
/// Nine seeds, all drawn from the Market identity, so every one of them moves
/// with the key draw -- and this same address is derived once here, once in
/// Claims and once in Custody on every Direct transaction. The founding is the
/// only party that ever derives it from seeds it has already authenticated, and
/// since `CoreState` carries its bump, the three readers reproduce it instead.
///
/// The derivation IS the check, exactly as `borrow_finalized_record_at` argues:
/// a wrong bump reproduces a different address, which the caller compares
/// against the account it was handed and refuses. For a non-canonical bump to
/// pass there would have to EXIST a Core-owned account, at the non-canonical
/// address, decoding to a valid state for this identity -- and Core creates
/// market states only at the canonical bump. Canonicality is enforced where the
/// account is made, not where it is read.
///
/// A state with no recorded bump takes the caller's mined hint instead, and
/// searches only if it was given neither. Both carriers land in the same
/// `create_program_address`, and the comparison above is what makes either one
/// safe: a founding that predates the tail costs a stranger nothing, because
/// the byte the founding did not write is a byte the caller can mine off chain
/// for free. See `StateBumpsV1` and `HotBumpHintsV1`.
///
/// The order is not arbitrary. The recorded bump wins where it exists because
/// it is the creator's own assertion, made once, on chain; the hint is the
/// fallback for state that has none. Neither is trusted -- a wrong value from
/// either carrier reproduces a different address and refuses.
pub(super) fn market_core_state_address_v2(
    state: CoreState,
    core_program: &Pubkey,
    hint: u8,
) -> Result<Pubkey, ProgramError> {
    let seeds = MarketCoreStateSeedsV2::new(state.identity);
    let base = seeds.as_slices();
    match state.bumps.market.or(hot_bump_hint_v1(hint)) {
        Some(bump) => {
            let bump_seed = [bump];
            Pubkey::create_program_address(
                &[
                    base[0], base[1], base[2], base[3], base[4], base[5], base[6], base[7],
                    base[8], &bump_seed,
                ],
                core_program,
            )
            .map_err(|_| TradingSbfError::Content.into())
        }
        None => Ok(Pubkey::find_program_address(&base, core_program).0),
    }
}

/// Borrow one finalized record at the canonical bumps its root recorded.
///
/// This is `borrow_finalized_record` with the two `find_program_address` calls
/// replaced by the two `create_program_address` calls they would have ended on.
/// Nothing about the conjunction is weakened: the derivation still has to
/// reproduce the account the caller supplied, still under this Market's
/// Registry and still from the schema and digest the caller is holding the
/// record to. What changes is only who paid for the search. A record's address
/// is an immutable fact about (schema, digest, Registry); the activation that
/// wrote this Market's root searched for it once, proved the account it found
/// was the finalized record, and wrote down the bump. A wrong bump reproduces a
/// different address and refuses here — the derivation IS the check — so the
/// stored bump is a hint that cannot lie, not an authority.
#[allow(clippy::too_many_arguments)]
pub(super) fn borrow_finalized_record_at<'a, 'info>(
    frame: HotFrameV3<'_, 'info>,
    raw: &'a AccountInfo<'info>,
    staging: &AccountInfo<'info>,
    schema: [u8; 32],
    digest: [u8; 32],
    raw_bump: u8,
    staging_bump: u8,
) -> Result<core::cell::Ref<'a, [u8]>, ProgramError> {
    let expected_raw = Pubkey::create_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest, &[raw_bump]],
        frame.registry.key,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let expected_staging = Pubkey::create_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &schema,
            &digest,
            &[staging_bump],
        ],
        frame.registry.key,
    )
    .map_err(|_| TradingSbfError::Content)?;
    borrow_record_against(frame, raw, staging, digest, expected_raw, expected_staging)
}

/// Borrow one finalized record, searching for both of its addresses.
///
/// This is the write-time form: the routes that must ESTABLISH a record's
/// canonical coordinate — activation, and the validated-artifact seal outer —
/// use it, once, and hand what they found to the readers. A hot action never
/// calls it; see [`borrow_finalized_record_at`].
pub(super) fn borrow_finalized_record<'a, 'info>(
    frame: HotFrameV3<'_, 'info>,
    raw: &'a AccountInfo<'info>,
    staging: &AccountInfo<'info>,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<core::cell::Ref<'a, [u8]>, ProgramError> {
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        frame.registry.key,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        frame.registry.key,
    )
    .0;
    borrow_record_against(frame, raw, staging, digest, expected_raw, expected_staging)
}

fn borrow_record_against<'a, 'info>(
    frame: HotFrameV3<'_, 'info>,
    raw: &'a AccountInfo<'info>,
    staging: &AccountInfo<'info>,
    digest: [u8; 32],
    expected_raw: Pubkey,
    expected_staging: Pubkey,
) -> Result<core::cell::Ref<'a, [u8]>, ProgramError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if raw.key != &expected_raw
        || raw.owner != frame.registry.key
        || raw.is_signer
        || raw.is_writable
        || raw.executable
        || hash(&data).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
        || staging.is_signer
        || staging.is_writable
        || staging.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(core::cell::Ref::map(data, |bytes| &**bytes))
}

#[derive(Clone, Copy)]
pub(super) struct HotFrameV3<'accounts, 'info> {
    pub(super) market: &'accounts AccountInfo<'info>,
    pub(super) root: &'accounts AccountInfo<'info>,
    pub(super) manifest_raw: &'accounts AccountInfo<'info>,
    pub(super) manifest_staging: &'accounts AccountInfo<'info>,
    pub(super) program_set_raw: &'accounts AccountInfo<'info>,
    pub(super) program_set_staging: &'accounts AccountInfo<'info>,
    pub(super) descriptor_raw: &'accounts AccountInfo<'info>,
    pub(super) descriptor_staging: &'accounts AccountInfo<'info>,
    pub(super) config_raw: &'accounts AccountInfo<'info>,
    pub(super) config_staging: &'accounts AccountInfo<'info>,
    pub(super) account_profile_raw: &'accounts AccountInfo<'info>,
    pub(super) account_profile_staging: &'accounts AccountInfo<'info>,
    pub(super) request_profile_raw: &'accounts AccountInfo<'info>,
    pub(super) request_profile_staging: &'accounts AccountInfo<'info>,
    pub(super) transition_raw: &'accounts AccountInfo<'info>,
    pub(super) transition_staging: &'accounts AccountInfo<'info>,
    pub(super) effect_raw: &'accounts AccountInfo<'info>,
    pub(super) effect_staging: &'accounts AccountInfo<'info>,
    pub(super) lifecycle_raw: &'accounts AccountInfo<'info>,
    pub(super) lifecycle_staging: &'accounts AccountInfo<'info>,
    pub(super) strategy_raw: &'accounts AccountInfo<'info>,
    pub(super) strategy_staging: &'accounts AccountInfo<'info>,
    pub(super) activation_cache: &'accounts AccountInfo<'info>,
    pub(super) core_program: &'accounts AccountInfo<'info>,
    pub(super) core_programdata: &'accounts AccountInfo<'info>,
    pub(super) trading_program: &'accounts AccountInfo<'info>,
    pub(super) trading_programdata: &'accounts AccountInfo<'info>,
    pub(super) registry: &'accounts AccountInfo<'info>,
    pub(super) rent: &'accounts AccountInfo<'info>,
    pub(super) instructions: &'accounts AccountInfo<'info>,
    pub(super) product_raw: &'accounts AccountInfo<'info>,
    pub(super) product_staging: &'accounts AccountInfo<'info>,
    pub(super) result_domain_raw: &'accounts AccountInfo<'info>,
    pub(super) result_domain_staging: &'accounts AccountInfo<'info>,
    pub(super) portfolio_raw: &'accounts AccountInfo<'info>,
    pub(super) portfolio_staging: &'accounts AccountInfo<'info>,
    pub(super) linked_basis_raw: &'accounts AccountInfo<'info>,
    pub(super) linked_basis_staging: &'accounts AccountInfo<'info>,
    pub(super) capability_seal: &'accounts AccountInfo<'info>,
}

/// Exact fixed-coordinate aliases admitted only by sealed Direct execution.
///
/// Seal materialization remains fully distinct. Once written, the immutable
/// seal owns the six finalized staging observations and Hot needs only each
/// live raw body plus that verdict. Keeping the fixed coordinate count stable
/// avoids a second wire ABI while removing six unique transaction locks.
/// The shape, from the ABI that also declares which families submit it.
///
/// This was a private copy of the same six pairs. The executor and the
/// producers have to agree exactly -- the gate compares with `!=` -- and two
/// spellings of one table is how they stop agreeing.
use dclutch_capability_program_contract::hot_v3::SEALED_EXECUTION_FIXED_ALIASES_V3;

fn is_sealed_execution_fixed_alias_v3(left: usize, right: usize) -> bool {
    SEALED_EXECUTION_FIXED_ALIASES_V3.contains(&(left, right))
}

/// Admit either the old fully-distinct frame or all six canonical seal-backed
/// aliases. Partial, wrong-pair, or seventh aliases refuse before any account
/// body is trusted.
pub(super) fn validate_hot_fixed_alias_shape_v3(accounts: &[AccountInfo<'_>]) -> Result<bool, ProgramError> {
    let fixed = accounts
        .get(..HOT_FIXED_ACCOUNT_COUNT_V3)
        .ok_or(TradingSbfError::Content)?;
    let sealed_alias_count = SEALED_EXECUTION_FIXED_ALIASES_V3
        .iter()
        .filter(|(raw, staging)| {
            fixed
                .get(*raw)
                .zip(fixed.get(*staging))
                .is_some_and(|(raw, staging)| raw.key == staging.key)
        })
        .count();
    if sealed_alias_count != 0 && sealed_alias_count != SEALED_EXECUTION_FIXED_ALIASES_V3.len() {
        return Err(TradingSbfError::Content.into());
    }
    for (left, account) in fixed.iter().enumerate() {
        for (offset, other) in fixed
            .get(left.saturating_add(1)..)
            .ok_or(TradingSbfError::Content)?
            .iter()
            .enumerate()
        {
            let right = left
                .checked_add(offset)
                .and_then(|value| value.checked_add(1))
                .ok_or(TradingSbfError::Content)?;
            if other.key == account.key && !is_sealed_execution_fixed_alias_v3(left, right) {
                return Err(TradingSbfError::Content.into());
            }
        }
    }
    Ok(sealed_alias_count == SEALED_EXECUTION_FIXED_ALIASES_V3.len())
}

impl<'accounts, 'info> HotFrameV3<'accounts, 'info> {
    fn from_accounts(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() < HOT_FIXED_ACCOUNT_COUNT_V3 {
            return Err(TradingSbfError::Content.into());
        }
        Ok(Self {
            market: account(accounts, HOT_MARKET_ACCOUNT_V3)?,
            root: account(accounts, HOT_ROOT_ACCOUNT_V3)?,
            manifest_raw: account(accounts, HOT_MANIFEST_RAW_ACCOUNT_V3)?,
            manifest_staging: account(accounts, HOT_MANIFEST_STAGING_ACCOUNT_V3)?,
            program_set_raw: account(accounts, HOT_PROGRAM_SET_RAW_ACCOUNT_V3)?,
            program_set_staging: account(accounts, HOT_PROGRAM_SET_STAGING_ACCOUNT_V3)?,
            descriptor_raw: account(accounts, HOT_DESCRIPTOR_RAW_ACCOUNT_V3)?,
            descriptor_staging: account(accounts, HOT_DESCRIPTOR_STAGING_ACCOUNT_V3)?,
            config_raw: account(accounts, HOT_CONFIG_RAW_ACCOUNT_V3)?,
            config_staging: account(accounts, HOT_CONFIG_STAGING_ACCOUNT_V3)?,
            account_profile_raw: account(accounts, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)?,
            account_profile_staging: account(accounts, HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3)?,
            request_profile_raw: account(accounts, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3)?,
            request_profile_staging: account(accounts, HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3)?,
            transition_raw: account(accounts, HOT_TRANSITION_RAW_ACCOUNT_V3)?,
            transition_staging: account(accounts, HOT_TRANSITION_STAGING_ACCOUNT_V3)?,
            effect_raw: account(accounts, HOT_EFFECT_RAW_ACCOUNT_V3)?,
            effect_staging: account(accounts, HOT_EFFECT_STAGING_ACCOUNT_V3)?,
            lifecycle_raw: account(accounts, HOT_LIFECYCLE_RAW_ACCOUNT_V3)?,
            lifecycle_staging: account(accounts, HOT_LIFECYCLE_STAGING_ACCOUNT_V3)?,
            strategy_raw: account(accounts, HOT_STRATEGY_RAW_ACCOUNT_V3)?,
            strategy_staging: account(accounts, HOT_STRATEGY_STAGING_ACCOUNT_V3)?,
            activation_cache: account(accounts, HOT_ACTIVATION_CACHE_ACCOUNT_V3)?,
            core_program: account(accounts, HOT_CORE_PROGRAM_ACCOUNT_V3)?,
            core_programdata: account(accounts, HOT_CORE_PROGRAMDATA_ACCOUNT_V3)?,
            trading_program: account(accounts, HOT_TRADING_PROGRAM_ACCOUNT_V3)?,
            trading_programdata: account(accounts, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3)?,
            registry: account(accounts, HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?,
            rent: account(accounts, HOT_RENT_SYSVAR_ACCOUNT_V3)?,
            instructions: account(accounts, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)?,
            product_raw: account(accounts, HOT_PRODUCT_RAW_ACCOUNT_V3)?,
            product_staging: account(accounts, HOT_PRODUCT_STAGING_ACCOUNT_V3)?,
            result_domain_raw: account(accounts, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3)?,
            result_domain_staging: account(accounts, HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3)?,
            portfolio_raw: account(accounts, HOT_PORTFOLIO_RAW_ACCOUNT_V3)?,
            portfolio_staging: account(accounts, HOT_PORTFOLIO_STAGING_ACCOUNT_V3)?,
            linked_basis_raw: account(accounts, HOT_LINKED_BASIS_RAW_ACCOUNT_V3)?,
            linked_basis_staging: account(accounts, HOT_LINKED_BASIS_STAGING_ACCOUNT_V3)?,
            capability_seal: account(accounts, HOT_CAPABILITY_SEAL_ACCOUNT_V3)?,
        })
    }

    pub(super) fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
        permits_fixed_market_union: bool,
    ) -> Result<Self, ProgramError> {
        let value = Self::from_accounts(accounts)?;
        if value.market.is_signer
            || (value.market.is_writable && !permits_fixed_market_union)
            || value.market.executable
            || value.root.is_signer
            || !value.root.is_writable
            || value.root.executable
            || value.trading_program.key != program_id
            || !value.trading_program.executable
            || value.trading_program.is_signer
            || value.trading_program.is_writable
            || !value.core_program.executable
            || value.core_program.is_signer
            || value.core_program.is_writable
            || !value.registry.executable
            || value.registry.is_signer
            || value.registry.is_writable
            || value.rent.key != &sysvar::rent::ID
            || value.rent.is_signer
            || value.rent.is_writable
            || value.rent.executable
        {
            return Err(TradingSbfError::Content.into());
        }
        validate_hot_fixed_alias_shape_v3(accounts)?;
        Ok(value)
    }

    pub(super) fn uses_sealed_execution_aliases(self) -> bool {
        SEALED_EXECUTION_FIXED_ALIASES_V3
            .iter()
            .all(|(raw, staging)| {
                let raw = match *raw {
                    HOT_DESCRIPTOR_RAW_ACCOUNT_V3 => self.descriptor_raw,
                    HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3 => self.account_profile_raw,
                    HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3 => self.request_profile_raw,
                    HOT_TRANSITION_RAW_ACCOUNT_V3 => self.transition_raw,
                    HOT_EFFECT_RAW_ACCOUNT_V3 => self.effect_raw,
                    HOT_LIFECYCLE_RAW_ACCOUNT_V3 => self.lifecycle_raw,
                    _ => return false,
                };
                let staging = match *staging {
                    HOT_DESCRIPTOR_STAGING_ACCOUNT_V3 => self.descriptor_staging,
                    HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3 => self.account_profile_staging,
                    HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3 => self.request_profile_staging,
                    HOT_TRANSITION_STAGING_ACCOUNT_V3 => self.transition_staging,
                    HOT_EFFECT_STAGING_ACCOUNT_V3 => self.effect_staging,
                    HOT_LIFECYCLE_STAGING_ACCOUNT_V3 => self.lifecycle_staging,
                    _ => return false,
                };
                raw.key == staging.key
            })
    }

    /// Parse the seal outer's fixed prefix: read-only root, writable seal.
    pub(super) fn parse_seal(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        let value = Self::from_accounts(accounts)?;
        if value.market.is_signer
            || value.market.is_writable
            || value.market.executable
            || value.root.is_signer
            || value.root.is_writable
            || value.root.executable
            || value.trading_program.key != program_id
            || !value.trading_program.executable
            || value.trading_program.is_signer
            || value.trading_program.is_writable
            || !value.core_program.executable
            || value.core_program.is_signer
            || value.core_program.is_writable
            || !value.registry.executable
            || value.registry.is_signer
            || value.registry.is_writable
            || value.rent.key != &sysvar::rent::ID
            || value.rent.is_signer
            || value.rent.is_writable
            || value.rent.executable
            || !value.capability_seal.is_writable
            || value.capability_seal.is_signer
            || value.capability_seal.executable
        {
            return Err(TradingSbfError::Content.into());
        }
        for (left, account) in accounts
            .get(..HOT_FIXED_ACCOUNT_COUNT_V3)
            .ok_or(TradingSbfError::Content)?
            .iter()
            .enumerate()
        {
            if accounts
                .get(left.saturating_add(1)..HOT_FIXED_ACCOUNT_COUNT_V3)
                .ok_or(TradingSbfError::Content)?
                .iter()
                .any(|other| other.key == account.key)
            {
                return Err(TradingSbfError::Content.into());
            }
        }
        Ok(value)
    }

    pub(super) fn parse_accelerator_readonly(
        trading_program: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        if accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
            || accounts
                .iter()
                .any(|account| account.is_signer || account.is_writable)
        {
            return Err(TradingSbfError::Content.into());
        }
        let value = Self::from_accounts(accounts)?;
        if value.market.executable
            || value.root.executable
            || value.trading_program.key != trading_program
            || !value.trading_program.executable
            || !value.core_program.executable
            || !value.registry.executable
            || value.rent.key != &sysvar::rent::ID
            || value.rent.executable
            || value.instructions.key != &sysvar::instructions::ID
            || value.instructions.executable
        {
            return Err(TradingSbfError::Content.into());
        }
        // The same sweep `parse` runs, from the same authority, rather than a
        // second copy of it. This function used to carry its own bare
        // pairwise-distinctness loop, which was the strictly older rule: it
        // refused the seal-backed alias shape that
        // `validate_hot_fixed_alias_shape_v3` exists to admit. Since every
        // AdmittedAot family reaches its accelerator through here, that copy
        // silently confined the alias shape to families that never take this
        // path, and it would have done so again for the next one.
        validate_hot_fixed_alias_shape_v3(accounts)?;
        Ok(value)
    }
}

pub(super) fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::Content.into())
}
