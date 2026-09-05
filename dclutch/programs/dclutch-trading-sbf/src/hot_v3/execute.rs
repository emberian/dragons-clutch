//! The Hot execution pipeline: authenticate the invocation, prepare every
//! mutation, run the child routes, verify, then commit last.

use super::*;

pub(super) const REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1: usize = 6;

/// Invocation facts authenticated from the current top-level instruction.
///
/// Registry continuation mode inserts one ephemeral admission signer before
/// strategy extras. It also permits physical privilege union on the fixed
/// Market observation; AccountProfile still owns the exact logical downgrade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AuthenticatedHotInvocationV3 {
    current_instruction: u16,
    pub(super) native_message_offset_bias: u16,
    pub(super) strategy_extras_start: usize,
    pub(super) permits_fixed_market_union: bool,
    pub(super) role_authentication: HotRoleAuthenticationV3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HotRoleAuthenticationV3 {
    ReauthenticateRegistry,
    AuthenticatedContinuation,
}

#[inline(never)]
pub(super) fn authenticate_hot_invocation_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    envelope: HotExecutionEnvelopeV3,
) -> Result<AuthenticatedHotInvocationV3, ProgramError> {
    let instructions = account(accounts, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)?;
    // The sysvar is compared in place under one borrow guard. Nothing read here
    // outlives the guard, and the whole comparison is complete before it is
    // dropped, so the bytes authenticated are exactly the bytes observed. A
    // nested self-CPI presents different data and metas than the top-level
    // record and is refused by the same two comparisons that authenticate the
    // direct case.
    // The sysvar record is compared in place, under one borrow guard held for
    // as long as any view read from it is alive. Nothing below performs a CPI,
    // so no reentrant invocation can run between the comparison that
    // authenticates these bytes and the admission they authorize; the guard
    // makes that structural rather than a comment. A nested self-CPI is refused
    // by the same two comparisons that authenticate the direct case, because
    // the sysvar record describes the top-level instruction and a nested
    // invocation presents different data and metas.
    let (current_instruction, sysvar) = borrow_authenticated_instructions_v1(instructions)?;
    let observed = SysvarInstructionV1::read(current_instruction, &sysvar)?;
    if observed.program_id() == program_id.as_array() {
        if observed.data() != instruction_data
            || observed.account_count() != accounts.len()
            || observed.metas().iter().zip(accounts).any(|(meta, info)| {
                meta.pubkey != info.key.as_array()
                    || meta.is_signer != info.is_signer
                    || meta.is_writable != info.is_writable
            })
        {
            return Err(TradingSbfError::NativeSignature.into());
        }
        return Ok(AuthenticatedHotInvocationV3 {
            current_instruction,
            native_message_offset_bias: 0,
            strategy_extras_start: HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3,
            permits_fixed_market_union: false,
            role_authentication: HotRoleAuthenticationV3::ReauthenticateRegistry,
        });
    }

    let registry = account(accounts, HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?;
    if observed.program_id() != registry.key.as_array() {
        return Err(TradingSbfError::NativeSignature.into());
    }
    if observed.data() != instruction_data {
        return Err(TradingSbfError::NativeSignature.into());
    }
    let activation = account(accounts, HOT_ACTIVATION_CACHE_ACCOUNT_V3)?;
    let activation_digest = {
        let bytes = activation
            .try_borrow_data()
            .map_err(|_| TradingSbfError::NativeSignature)?;
        ContentId::new(hash(&bytes).to_bytes()).map_err(|_| TradingSbfError::NativeSignature)?
    };
    let hot_digest =
        ContentId::new(hash(instruction_data).to_bytes()).map_err(|_| TradingSbfError::Content)?;
    let request = RegistryContinuationRequestV1::new_core_trading_hot(
        ContentId::new(envelope.release_set()).map_err(|_| TradingSbfError::Content)?,
        activation_digest,
        hot_digest,
        u32::try_from(instruction_data.len()).map_err(|_| TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::NativeSignature)?;

    let admission = account(accounts, HOT_FIXED_ACCOUNT_COUNT_V3)?;
    if !admission.is_signer
        || admission.is_writable
        || admission.executable
        || admission.owner != &system_program::ID
        || !admission.data_is_empty()
        || admission.lamports() != 0
        || accounts
            .iter()
            .filter(|info| info.key == admission.key)
            .count()
            != 1
    {
        return Err(TradingSbfError::Release.into());
    }
    let batch = request
        .role_batch_request()
        .map_err(|_| TradingSbfError::NativeSignature)?;
    let batch_digest = ContentId::new(hash(&batch.to_bytes()).to_bytes())
        .map_err(|_| TradingSbfError::NativeSignature)?;
    let seeds =
        RegistryContinuationAdmissionSeedsV1::new(request, activation.key.to_bytes(), batch_digest)
            .map_err(|_| TradingSbfError::NativeSignature)?;
    let release = seeds.release_set();
    let cache = seeds.activation_cache();
    let batch = seeds.batch_request_digest();
    let mask = seeds.role_mask();
    let role = seeds.continuation_role();
    let digest = seeds.continuation_digest();
    let expected_admission = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            batch.as_slice(),
            mask.as_slice(),
            role.as_slice(),
            digest.as_slice(),
        ],
        registry.key,
    )
    .0;
    if expected_admission != *admission.key {
        return Err(TradingSbfError::Release.into());
    }

    let outer = observed.metas_range(0, REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1)?;
    let expected_outer = [
        account(accounts, HOT_ACTIVATION_CACHE_ACCOUNT_V3)?.key,
        account(accounts, HOT_CORE_PROGRAM_ACCOUNT_V3)?.key,
        account(accounts, HOT_CORE_PROGRAMDATA_ACCOUNT_V3)?.key,
        account(accounts, HOT_TRADING_PROGRAM_ACCOUNT_V3)?.key,
        account(accounts, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3)?.key,
        admission.key,
    ];
    if outer
        .iter()
        .zip(expected_outer)
        .any(|(meta, key)| meta.pubkey != key.as_array() || meta.is_signer || meta.is_writable)
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    let observed_nested = observed.metas_from(REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1)?;
    if observed_nested.len() != accounts.len()
        || observed_nested
            .iter()
            .zip(accounts)
            .enumerate()
            .any(|(index, (meta, info))| {
                meta.pubkey != info.key.as_array()
                    || meta.is_writable != info.is_writable
                    || if index == HOT_FIXED_ACCOUNT_COUNT_V3 {
                        meta.is_signer
                    } else {
                        meta.is_signer != info.is_signer
                    }
            })
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    Ok(AuthenticatedHotInvocationV3 {
        current_instruction,
        native_message_offset_bias: 0,
        strategy_extras_start: HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3
            .checked_add(1)
            .ok_or(TradingSbfError::Content)?,
        permits_fixed_market_union: true,
        role_authentication: HotRoleAuthenticationV3::AuthenticatedContinuation,
    })
}

/// Common authenticated artifact/interpreter tail shared by live-Market Hot
/// and the exact Series pre-Market expiry mode.
///
/// Each caller owns how its logical Market facts, root, and Product graph are
/// authenticated. From this boundary onward there is one semantic owner for
/// release selection, sealed artifacts, the generic interpreter, child CPIs,
/// and commit-last persistence.
pub(super) struct AuthenticatedHotPreludeV3<'program, 'request, 'accounts, 'info> {
    pub(super) program_id: &'program Pubkey,
    pub(super) accounts: &'accounts [AccountInfo<'info>],
    pub(super) instruction_data: &'request [u8],
    pub(super) family_request: &'request [u8],
    pub(super) envelope: HotExecutionEnvelopeV3,
    pub(super) invocation: AuthenticatedHotInvocationV3,
    pub(super) frame: Box<HotFrameV3<'accounts, 'info>>,
    pub(super) request_digest: [u8; 32],
    pub(super) root_prestate: [u8; 32],
    pub(super) market: AuthenticatedLogicalMarketV3,
    pub(super) root: Box<AuthenticatedRootV3>,
    pub(super) rent: Rent,
    pub(super) product_runtime_v3: Box<AuthenticatedProductRuntimeV3<'accounts, 'info>>,
    pub(super) authenticated_series_expiry_replay: bool,
    pub(super) authenticated_series_expiry_rent_credit: [u8; 32],
}

/// Exact Market facts consumed by the family-neutral Hot tail.
///
/// Ordinary execution copies these from authenticated persisted Core state.
/// The Series pre-Market mode instead supplies the same facts from its
/// independently authenticated immutable projection and founding permit; it
/// never invents a `CoreState` for an account which does not exist yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AuthenticatedLogicalMarketV3 {
    pub(super) identity: MarketIdentity,
    pub(super) rent_beneficiary: CoreIdentity,
}

impl AuthenticatedLogicalMarketV3 {
    pub(super) fn from_live(market: &CoreState) -> Self {
        Self {
            identity: market.identity,
            rent_beneficiary: market.rent_beneficiary,
        }
    }
}

#[inline(never)]
pub(super) fn authenticate_and_execute_hot_v3(
    prepared: &AuthenticatedHotPreludeV3<'_, '_, '_, '_>,
) -> Result<(), ProgramError> {
    let program_id = prepared.program_id;
    let accounts = prepared.accounts;
    let instruction_data = prepared.instruction_data;
    let family_request = prepared.family_request;
    let envelope = prepared.envelope;
    let invocation = prepared.invocation;
    let frame: &HotFrameV3<'_, '_> = &prepared.frame;
    let request_digest = prepared.request_digest;
    let root_prestate = prepared.root_prestate;
    let market = &prepared.market;
    let root = &prepared.root;
    let rent = &prepared.rent;
    let product_runtime_v3 = &prepared.product_runtime_v3;
    let authenticated_series_expiry_replay = prepared.authenticated_series_expiry_replay;
    let authenticated_series_expiry_rent_credit = prepared.authenticated_series_expiry_rent_credit;
    let context = &root.context;
    let product_runtime = product_runtime_v3.runtime;
    hot_cu_checkpoint!("root-product");
    // The three record coordinates below are read, not searched for. See
    // `borrow_finalized_record_at`.
    let record_bumps = context.record_bumps();
    let manifest_data = borrow_finalized_record_at(
        *frame,
        frame.manifest_raw,
        frame.manifest_staging,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        context.selection().manifest().to_bytes(),
        record_bumps.manifest_raw(),
        record_bumps.manifest_staging(),
    )?;
    let entry = authenticate_manifest_entry_boxed_v3(&manifest_data, context)?;

    let capability_release = context.selection().capability_release().to_bytes();
    let program_set_data = borrow_finalized_record_at(
        *frame,
        frame.program_set_raw,
        frame.program_set_staging,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        capability_release,
        context.selection().capability_release_raw_bump(),
        context.selection().capability_release_staging_bump(),
    )?;
    // The record's own authentication is the single owner of its content
    // digest: `borrow_finalized_record` refuses unless `hash(program_set_data)`
    // is exactly `capability_release`, so the selected identity and the
    // authenticated digest are one value and hashing the record again here only
    // recomputed it.
    let program_set = CapabilityProgramSetV2::decode_selected(
        capability_release,
        capability_release,
        &program_set_data,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let selected_entry = program_set
        .select_entry(family_request)
        .map_err(|_| TradingSbfError::Content)?;
    let selected_descriptor = selected_entry.descriptor();
    if selected_descriptor.schema().to_bytes() != PROGRAM_SCHEMA_ID_V4 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let selected_program = selected_descriptor.program();
    let selected_action = selected_entry.selector();

    // A family that spends the write-once artifact verdict authenticated below
    // carries its six sealed staging coordinates as the matching raw account
    // again: the seal is the durable proof that the real staging cursor was
    // vacant when this exact raw body was admitted, and Registry finalization
    // is monotone, so that proof cannot go stale. Families that do not keep the
    // fully-distinct frame and observe each cursor live.
    //
    // The table is the ABI's, not this function's --
    // `capability_program_contract::hot_v3` -- so the executor, the bundle
    // builders and the operators read one declaration instead of each spelling
    // the rule again. `!=` and not `||`: the wrong shape for the family refuses
    // in either direction, so the shape is the family's and never the
    // submitter's choice.
    let selected_kind = context.selection().kind().to_bytes();
    if frame.uses_sealed_execution_aliases()
        != hot_frame_uses_sealed_execution_aliases_v3(selected_kind, selected_action)
    {
        return Err(TradingSbfError::Content.into());
    }

    // Decision 0005: the validated-artifact seal for exactly this descriptor,
    // this action, this authenticated Trading interpreter release and this
    // Market-selected Registry. Authenticated before any artifact it names is
    // read, and consulted only for addresses this Program derived once from
    // the same seeds and for verdicts about bytes still pinned live by their
    // own digest.
    let seal_data = frame
        .capability_seal
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let seal = authenticate_capability_seal_v3(
        program_id,
        *frame,
        selected_descriptor.schema().to_bytes(),
        selected_program.to_bytes(),
        selected_action,
        root.trading_semantic_release,
        &seal_data,
    )?;

    let descriptor_data = borrow_sealed_record(
        *frame,
        seal,
        SealedRoleV1::Descriptor,
        frame.descriptor_raw,
        frame.descriptor_staging,
        selected_descriptor.schema().to_bytes(),
        selected_program.to_bytes(),
    )?;
    if descriptor_data.len() != CAPABILITY_PROGRAM_V4_BYTES {
        return Err(TradingSbfError::Content.into());
    }
    let descriptor = decode_capability_program_boxed_v3(&descriptor_data)?;
    authenticate_descriptor_root_selection(&descriptor, context, &entry)?;

    let config_data = borrow_finalized_record_at(
        *frame,
        frame.config_raw,
        frame.config_staging,
        descriptor.config_schema().to_bytes(),
        context.selection().config().to_bytes(),
        record_bumps.config_raw(),
        record_bumps.config_staging(),
    )?;
    let direct_config = if selected_kind == DIRECT_SUCCESSOR_KIND_ID_V3
        && direct_action_crosschecks_against_config_v3(selected_action)
    {
        let config_id = context.selection().config().to_bytes();
        Some(
            DirectExecutionConfigV1::decode_selected(config_id, config_id, &config_data)
                .map_err(|_| TradingSbfError::Content)?,
        )
    } else {
        None
    };
    // The config record needs no digest of its own here either: the borrow
    // above refuses unless `hash(config_data)` is the selected config identity,
    // so re-hashing it and comparing the result against that identity could
    // only ever agree.
    drop(config_data);
    require_common_projection_bindings_v3(CommonProjectionBindingsV3 {
        selected_config: context.selection().config().to_bytes(),
        selected_product_record: market.identity.product_record.to_bytes(),
        authenticated_product_record: product_runtime.product_record.content_digest.to_bytes(),
        market_product: market.identity.product_id.to_bytes(),
        runtime_product: product_runtime.product_id.to_bytes(),
        product_semantic_basis: product_runtime.liability_basis_id.to_bytes(),
        authenticated_semantic_basis: product_runtime_v3.semantic_basis_id.to_bytes(),
        authenticated_linked_basis: product_runtime_v3
            .linked_basis_record
            .content_digest
            .to_bytes(),
    })?;
    let lifecycle_data = borrow_sealed_record(
        *frame,
        seal,
        SealedRoleV1::LifecyclePolicy,
        frame.lifecycle_raw,
        frame.lifecycle_staging,
        descriptor.lifecycle().schema().to_bytes(),
        descriptor.lifecycle().program().to_bytes(),
    )?;
    // R2 -- `derivation_policy == lifecycle().program()` -- is GONE, and what it
    // restated was already authenticated more strongly two statements up.
    //
    // `lifecycle().program()` is the lifecycle record's CONTENT DIGEST. The
    // `borrow_finalized_record` above refuses unless `hash(&data) == digest`
    // (`hot_v3.rs:13437`) at the Registry PDA derived from
    // `[RAW_RECORD_PDA_SEED_V1, schema, digest]`, and `sealed_token` below binds
    // those exact bytes to the execution seal. R2 authenticated nothing on top of
    // that; it only demanded that ONE field be a per-action digest and a per-root
    // constant at the same time, which no descriptor can satisfy once a family
    // has more than one action -- `derivation_policy` is per-root by
    // construction and the lifecycle digest is per-action.
    //
    // What still binds the descriptor to its manifest entry is untouched:
    // `validate_selection` compares `kind`, `release_id`, `config_id`,
    // `capacity_profile` and `root_schema`. The lifecycle SCHEMA admission below
    // is untouched too. This is a re-proof on the other side, not a weakening.
    //
    // The host half is `a153f08e`; this is the runtime half, and the two are one
    // repair. R2 spans two ELFs -- dropping it in Trading alone leaves the dealer
    // accelerator refusing `0xd001` here.
    if descriptor.lifecycle().schema().to_bytes() != SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let lifecycle_token = sealed_token(
        seal,
        SealedRoleV1::LifecyclePolicy,
        descriptor.lifecycle().schema().to_bytes(),
        descriptor.lifecycle().program().to_bytes(),
        &lifecycle_data,
    )?;
    let lifecycle = StateLifecyclePolicyV5::from_sealed(&lifecycle_data, lifecycle_token)
        .map_err(|_| TradingSbfError::Content)?;

    let account_profile_data = borrow_sealed_record(
        *frame,
        seal,
        SealedRoleV1::AccountProfile,
        frame.account_profile_raw,
        frame.account_profile_staging,
        descriptor.account_profile().schema().to_bytes(),
        descriptor.account_profile().program().to_bytes(),
    )?;
    let account_profile_token = sealed_token(
        seal,
        SealedRoleV1::AccountProfile,
        descriptor.account_profile().schema().to_bytes(),
        descriptor.account_profile().program().to_bytes(),
        &account_profile_data,
    )?;
    let account_schema = descriptor.account_profile().schema().to_bytes();
    let (account_profile, funding_profile) = if account_schema == ACCOUNT_PROFILE_SCHEMA_ID_V2 {
        (
            AccountProfileV2::from_sealed(&account_profile_data, account_profile_token)
                .map_err(|_| TradingSbfError::Content)?,
            None,
        )
    } else if account_schema == ACCOUNT_PROFILE_SCHEMA_ID_V3 {
        let funding = AccountProfileV3::from_sealed(&account_profile_data, account_profile_token)
            .map_err(|_| TradingSbfError::Content)?;
        (funding.base(), Some(funding))
    } else {
        return Err(TradingSbfError::UnsupportedContent.into());
    };
    // One validated join for the whole execution: the lifecycle preplan runs a
    // batch of plans over these same two immutable artifacts, twice, and the
    // planner otherwise re-derives this join for every planned state. The join
    // is a fact about the pair, so the seal owns it and mints it from its own
    // two tokens.
    let profile_join = if let Some(funding) = funding_profile {
        lifecycle
            .validate_account_profile_with_external_funding_join(funding)
            .map_err(|_| TradingSbfError::Content)?
    } else {
        lifecycle
            .sealed_account_profile_join(
                account_profile,
                seal.authenticate_profile_join(lifecycle_token, account_profile_token)
                    .map_err(|_| TradingSbfError::Content)?,
            )
            .map_err(|_| TradingSbfError::Content)?
    };

    let request_profile_data = borrow_sealed_record(
        *frame,
        seal,
        SealedRoleV1::RequestProfile,
        frame.request_profile_raw,
        frame.request_profile_staging,
        descriptor.request_profile().schema().to_bytes(),
        descriptor.request_profile().program().to_bytes(),
    )?;
    let request_profile_token = sealed_token(
        seal,
        SealedRoleV1::RequestProfile,
        descriptor.request_profile().schema().to_bytes(),
        descriptor.request_profile().program().to_bytes(),
        &request_profile_data,
    )?;
    let request_profile =
        decode_sealed_request_profile(*descriptor, &request_profile_data, request_profile_token)?;

    let (strategy, strategy_extras_end) = authenticate_strategy_from_sealed_boxed_v3(
        &frame,
        accounts,
        *context,
        selected_program,
        descriptor.as_ref(),
        invocation.strategy_extras_start,
    )?;

    let transition_data = borrow_sealed_record(
        *frame,
        seal,
        SealedRoleV1::TransitionProgram,
        frame.transition_raw,
        frame.transition_staging,
        descriptor.transition().schema().to_bytes(),
        descriptor.transition().program().to_bytes(),
    )?;
    if descriptor.transition().schema().to_bytes() != TRANSITION_SCHEMA_ID_V3
        || strategy.strategy().transition_schema() != descriptor.transition().schema()
        || strategy.strategy().transition_program() != descriptor.transition().program()
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let transition_token = sealed_token(
        seal,
        SealedRoleV1::TransitionProgram,
        descriptor.transition().schema().to_bytes(),
        descriptor.transition().program().to_bytes(),
        &transition_data,
    )?;
    let transition = TransitionProgramV3::from_sealed(&transition_data, transition_token)
        .map_err(|_| TradingSbfError::Content)?;

    let effect_data = borrow_sealed_record(
        *frame,
        seal,
        SealedRoleV1::EffectProgram,
        frame.effect_raw,
        frame.effect_staging,
        descriptor.effect().schema().to_bytes(),
        descriptor.effect().program().to_bytes(),
    )?;
    let effect_token = sealed_token(
        seal,
        SealedRoleV1::EffectProgram,
        descriptor.effect().schema().to_bytes(),
        descriptor.effect().program().to_bytes(),
        &effect_data,
    )?;
    let effect = decode_sealed_effect_v4(
        descriptor.effect().schema().to_bytes(),
        &effect_data,
        effect_token,
    )?;
    if effect.funding().is_some() != funding_profile.is_some() {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    require_funding_profile_join_v5(effect, funding_profile)?;
    // The ownership conjunction is a fact about four immutable artifacts and
    // the selected action, and the action is a seed of this seal.
    let sealed_ownership = seal
        .authenticate_static_ownership(
            account_profile_token,
            lifecycle_token,
            request_profile_token,
            transition_token,
        )
        .map_err(|_| TradingSbfError::Content)?;
    hot_cu_checkpoint!("artifacts-strategy-effect");

    execute_authenticated_hot_v3(AuthenticatedHotExecutionV3 {
        program_id,
        accounts,
        instruction_data,
        family_request,
        envelope,
        invocation,
        frame,
        request_digest,
        root_prestate,
        market,
        root,
        rent: rent.clone(),
        product_runtime_v3,
        selected_program,
        selected_action,
        selected_kind,
        direct_config,
        descriptor: &descriptor,
        lifecycle,
        account_profile,
        funding_profile,
        profile_join,
        request_profile,
        strategy: &strategy,
        strategy_extras_end,
        transition,
        effect,
        sealed_ownership,
        authenticated_series_expiry_replay,
        authenticated_series_expiry_rent_credit,
    })
}

/// Everything the authentication half proved, handed to the half that executes
/// it.
///
/// The boundary is not cosmetic. SBPF v0 gives every function a static
/// 4,096-byte frame, and one function holding both halves' live values does not
/// fit: the artifact half alone peaks at 2,176 bytes and the execution half
/// needs 2,240 more. Splitting them also confines the nineteen `RefCell` borrow
/// guards and five seal tokens to the half that authenticates against them --
/// none of them crosses -- so the execution half cannot read an artifact whose
/// seal it did not receive.
pub(super) struct AuthenticatedHotExecutionV3<'a, 'accounts, 'info, 'artifact> {
    program_id: &'a Pubkey,
    accounts: &'accounts [AccountInfo<'info>],
    instruction_data: &'artifact [u8],
    family_request: &'artifact [u8],
    envelope: HotExecutionEnvelopeV3,
    invocation: AuthenticatedHotInvocationV3,
    frame: &'a HotFrameV3<'accounts, 'info>,
    request_digest: [u8; 32],
    root_prestate: [u8; 32],
    market: &'a AuthenticatedLogicalMarketV3,
    root: &'a AuthenticatedRootV3,
    rent: Rent,
    product_runtime_v3: &'a AuthenticatedProductRuntimeV3<'accounts, 'info>,
    selected_program: ContentId,
    selected_action: u32,
    selected_kind: [u8; 32],
    direct_config: Option<DirectExecutionConfigV1>,
    descriptor: &'a CapabilityProgramV4,
    lifecycle: StateLifecyclePolicyV5<'artifact>,
    account_profile: AccountProfileV2<'artifact>,
    funding_profile: Option<AccountProfileV3<'artifact>>,
    profile_join: ValidatedProfileJoinV3<'artifact>,
    request_profile: RequestProfileKindV3<'artifact>,
    strategy: &'a AuthenticatedExecutionStrategyV2,
    strategy_extras_end: usize,
    transition: TransitionProgramV3<'artifact>,
    effect: SelectedEffectProgramV4<'artifact>,
    sealed_ownership: SealedStaticOwnershipV1<'artifact>,
    authenticated_series_expiry_replay: bool,
    authenticated_series_expiry_rent_credit: [u8; 32],
}

/// Run the ten execution phases over artifacts that are already authenticated.
#[inline(never)]
pub(super) fn execute_authenticated_hot_v3(
    prepared: AuthenticatedHotExecutionV3<'_, '_, '_, '_>,
) -> Result<(), ProgramError> {
    let AuthenticatedHotExecutionV3 {
        program_id,
        accounts,
        instruction_data,
        family_request,
        envelope,
        invocation,
        frame,
        request_digest,
        root_prestate,
        market,
        root,
        rent,
        product_runtime_v3,
        selected_program,
        selected_action,
        selected_kind,
        direct_config,
        descriptor,
        lifecycle,
        account_profile,
        funding_profile,
        profile_join,
        request_profile,
        strategy,
        strategy_extras_end,
        transition,
        effect,
        sealed_ownership,
        authenticated_series_expiry_replay,
        authenticated_series_expiry_rent_credit,
    } = prepared;
    let context = &root.context;
    let immutable_root_header = &root.immutable_header;
    let product_runtime = product_runtime_v3.runtime;
    let product_outcome_count = product_runtime.outcome_count;
    let strategy_extras = accounts
        .get(invocation.strategy_extras_start..strategy_extras_end)
        .ok_or(TradingSbfError::Content)?;

    let provisional_scalar_count = effect
        .scalar_count(product_outcome_count)
        .map_err(|_| TradingSbfError::Content)?;
    let provisional_identity_count = effect
        .identity_count(product_outcome_count)
        .map_err(|_| TradingSbfError::Content)?;
    let StrategyFrameSpanV3 {
        shadow_caller_authority,
        admitted_caller_authorities,
        admitted_output_page,
        runtime_start,
    } = carve_strategy_frame_span_v3(
        accounts,
        &strategy,
        invocation.strategy_extras_start,
        strategy_extras_end,
        provisional_scalar_count,
        provisional_identity_count,
    )?;
    let expected_shadow_runtime = HOT_SHADOW_RUNTIME_ACCOUNTS_START_V3
        .checked_add(
            invocation
                .strategy_extras_start
                .checked_sub(HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3)
                .ok_or(TradingSbfError::Content)?,
        )
        .ok_or(TradingSbfError::Content)?;
    if shadow_caller_authority.is_some() && runtime_start != expected_shadow_runtime {
        return Err(TradingSbfError::Content.into());
    }

    require_shadow_declares_no_trusted_slot_v1(
        strategy.strategy().disposition(),
        account_profile.trusted_environment(),
    )?;
    let trusted_environment = observe_trusted_environment_v3(account_profile, program_id)?;
    let dynamic_spans = authenticate_dynamic_span_widths_v3(
        account_profile,
        request_profile,
        effect,
        strategy.strategy().disposition(),
        product_outcome_count,
        family_request,
        request_digest,
        trusted_environment,
        provisional_scalar_count,
        provisional_identity_count,
    )?;

    let runtime_accounts = expand_runtime_accounts_v3(
        account_profile,
        product_outcome_count,
        &dynamic_spans.widths,
        [
            frame.root,
            frame.config_raw,
            frame.product_raw,
            frame.portfolio_raw,
            frame.linked_basis_raw,
        ],
        accounts
            .get(runtime_start..)
            .ok_or(TradingSbfError::Content)?,
    )?;
    let input_scratch_pages = authenticated_input_scratch_pages_v3(
        account_profile,
        &dynamic_spans.widths,
        dynamic_spans.transport_span,
        &runtime_accounts,
    )?;
    if runtime_accounts.len() > MAX_HOT_RUNTIME_ACCOUNTS_V3 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    // The borrow guards and the observation bank they back are the two banks
    // this execution allocates EARLY and stops reading EARLIEST -- and they
    // were the hole W2o measured: 5,968 bytes dying underneath eighteen
    // kilobytes that do not. They are the scratch region's whole reason to
    // exist. Both come off the heap's high end, so the reclaim below does not
    // depend on the order anything else was allocated in, and neither can
    // outlive `scratch`: `ScratchVecV1` borrows it.
    let scratch = HeapScratchRegionV1::open()?;
    // Exact capacity, not `collect::<Result<Vec<_>, _>>()`. A fallible collect
    // reports a zero lower bound, so the SBF bump allocator - which never frees
    // - is walked through the whole doubling ladder and charges several times
    // the live width for every fallible bank on this path.
    hot_heap_mark!("runtime-accounts");
    let mut runtime_data = ScratchVecV1::with_capacity(&scratch, runtime_accounts.len())?;
    for account in &runtime_accounts {
        runtime_data.push(
            account
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Content)?,
        )?;
    }
    hot_heap_mark!("runtime-data");
    let projected_tail_count = project_tail_count(account_profile, product_outcome_count)?;
    require_tail_count_agreement_v3(product_outcome_count, projected_tail_count)?;
    // A profile that projects no tail count operates at width zero, which is
    // what a fixed topology means. Every item span downstream is then empty,
    // which is the correct geometry rather than a degenerate one.
    let tail_count = projected_tail_count.unwrap_or(0);
    // Representatives are resolved before the observation bank because the
    // logical projection key of an aliased coordinate is its representative's,
    // not its own.
    let aliases = representative_coordinates_v3(
        account_profile,
        tail_count,
        &dynamic_spans.widths,
        runtime_accounts.len(),
    )?;
    hot_heap_mark!("aliases");
    let projected_keys = Box::new(LogicalProjectionKeysV3 {
        selected_config: context.selection().config().to_bytes(),
        product_root: product_runtime.product_record.content_digest.to_bytes(),
        portfolio: product_runtime.portfolio_record.content_digest.to_bytes(),
        linked_basis: product_runtime_v3
            .linked_basis_record
            .content_digest
            .to_bytes(),
    });
    let selected_config_is_variable = projected_account_uses_variable_marker_v3(
        account_profile,
        HOT_SELECTED_CONFIG_LOGICAL_ACCOUNT_V3,
    )?;
    let linked_basis_is_variable = projected_account_uses_variable_marker_v3(
        account_profile,
        HOT_LINKED_BASIS_LOGICAL_ACCOUNT_V3,
    )?;
    // THE PRODUCT RECORD'S DATA DIGEST, supplied as an observation because the
    // AccountProfile interpreter does not hash and must not start: a digest is
    // an adapter-established fact, like the key and the owner beside it, and
    // `ProjectDataDigest` projects it.
    //
    // It is computed for THIS coordinate only. Hashing every observed account
    // would be a per-account SHA-256 on the CU-bound hot path for a fact almost
    // no profile asks for, and the Product record is a fixed Hot-frame
    // coordinate rather than a family one -- the same kind of family-neutral
    // special case the selected-config and linked-basis markers above already
    // are. `hash` over the WHOLE account data is the convention the consumer
    // recomputes; a Registry record's `content_digest` is over record content
    // and is deliberately not what goes here.
    let product_record_data_digest = runtime_data
        .get(HOT_RUNTIME_PRODUCT_COORDINATE_V3)
        .map(|data| hash(data.as_ref()).to_bytes());
    let mut observations = ScratchVecV1::with_capacity(&scratch, runtime_accounts.len())?;
    for (coordinate, (account, data)) in
        runtime_accounts.iter().zip(runtime_data.iter()).enumerate()
    {
        let key = logical_projection_key_v3(
            *aliases.get(coordinate).unwrap_or(&coordinate),
            account.key,
            &projected_keys,
        );
        observations.push(
            if (coordinate == HOT_SELECTED_CONFIG_LOGICAL_ACCOUNT_V3 && selected_config_is_variable)
                || (coordinate == HOT_LINKED_BASIS_LOGICAL_ACCOUNT_V3 && linked_basis_is_variable)
            {
                // The Product-runtime reader above authenticated Registry
                // finality, schema, content digest, and either the selected
                // immutable config or Product-owned semantic basis before
                // this observation is constructed.
                AccountObservationV1::new_adapter_authenticated_variable_data(
                    key,
                    account.owner.as_array(),
                    account.lamports(),
                    data.as_ref(),
                    account.is_signer,
                    account.is_writable,
                    account.executable,
                )
            } else {
                let observation = AccountObservationV1::new(
                    key,
                    account.owner.as_array(),
                    account.lamports(),
                    data.as_ref(),
                    account.is_signer,
                    account.is_writable,
                    account.executable,
                );
                match (coordinate, product_record_data_digest.as_ref()) {
                    (HOT_RUNTIME_PRODUCT_COORDINATE_V3, Some(digest)) => {
                        observation.with_adapter_data_digest(digest)
                    }
                    _ => observation,
                }
            },
        )?;
    }
    hot_cu_checkpoint!("runtime-observations");

    require_geometry(
        account_profile,
        request_profile,
        transition,
        effect,
        tail_count,
        family_request,
        runtime_accounts.len(),
        &dynamic_spans.widths,
        dynamic_spans.effect_span_extension.as_ref(),
    )?;
    let lifecycle_width = lifecycle_semantic_prefix_width_v3(
        account_profile,
        tail_count,
        &dynamic_spans.widths,
        runtime_accounts.len(),
    )?;
    let lifecycle_observations = observations
        .get(..lifecycle_width)
        .ok_or(TradingSbfError::Content)?;
    let lifecycle_runtime_accounts = runtime_accounts
        .get(..lifecycle_width)
        .ok_or(TradingSbfError::Content)?;
    let lifecycle_aliases = aliases
        .get(..lifecycle_width)
        .ok_or(TradingSbfError::Content)?;
    let scalar_count = effect
        .scalar_count(tail_count)
        .map_err(|_| TradingSbfError::Content)?;
    let identity_count = effect
        .identity_count(tail_count)
        .map_err(|_| TradingSbfError::Content)?;
    let request_bytes = effect
        .request_bytes(tail_count)
        .map_err(|_| TradingSbfError::Content)?;
    if scalar_count > MAX_HOT_SCALARS_V3
        || identity_count > MAX_HOT_IDENTITIES_V3
        || request_bytes > MAX_HOT_REQUEST_BYTES_V3
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    // THE FRAME'S CARVING AND THE TRANSPORT'S CHUNKING ARE TWO AUTHORS, joined
    // here. The carving above asked `admitted_caller_authority_count_v3` for a
    // caller-authority span using the effect's widths at
    // `product_outcome_count`, because it runs before any bank exists. The bank
    // is built at `tail_count`, and `require_tail_count_agreement_v3` lets those
    // differ: a profile that projects NO tail count leaves `tail_count` zero
    // while the frame was carved at an outcome count of at least two. Nothing
    // said the two had to agree.
    if admitted_caller_authorities.is_some() {
        require_admitted_bank_matches_frame_v3(
            (scalar_count, identity_count),
            (
                effect
                    .scalar_count(product_outcome_count)
                    .map_err(|_| TradingSbfError::Content)?,
                effect
                    .identity_count(product_outcome_count)
                    .map_err(|_| TradingSbfError::Content)?,
            ),
        )?;
    }
    let current_rent_quotes =
        authenticate_current_rent_quotes_v5(lifecycle, &rent, selected_action)?;
    hot_heap_mark!("rent-quotes");
    hot_cu_checkpoint!("p5-geometry-rent");

    // ONE BYTE PER COORDINATE, ALLOCATED HERE BECAUSE IT OUTLIVES THE SCRATCH.
    // The account projection below decodes every coordinate's rule -- and, for
    // a route alias, its representative's -- and used to throw both away, after
    // which `project_hot_effects_v3` decoded all of them again to keep the
    // permission byte. It is filled where the rules are already in hand, and
    // survives the observation bank's release the way the account inputs do.
    let mut effect_permissions =
        try_projection_bank_v3(&AccountPermission::read_only(), observations.len())?;
    // Destructured at the call: every one of the four banks borrows the
    // scratch region, and a named binding of the whole struct would own that
    // borrow until the end of the function, past the release.
    let ProjectedRequestRegistersV3 {
        scalars: request_output_scalars,
        identities: request_output_identities,
        spare_scalars,
        spare_identities,
    } = project_account_and_request_registers_v3(
        &scratch,
        invocation.current_instruction,
        invocation.native_message_offset_bias,
        instruction_data,
        *frame,
        account_profile,
        request_profile,
        lifecycle,
        profile_join,
        selected_action,
        &current_rent_quotes,
        &dynamic_spans.widths,
        tail_count,
        &observations,
        &mut effect_permissions,
        family_request,
        request_digest,
        trusted_environment,
        product_outcome_count,
        scalar_count,
        identity_count,
    )?;
    hot_heap_mark!("request-registers");
    hot_cu_checkpoint!("p5-request-registers");
    // THE VERDICT COVERS THE RECORD THE SEAL PINNED, NOT ITS INTERIOR.
    // `authenticate_static_ownership` is minted from `account_profile_token`,
    // and that token names `&account_profile_data` -- the complete Registry
    // record. For schema V2 the profile IS that record. For schema V3 the
    // profile handed on is `funding.base()`, an interior slice starting after
    // the V3 header and its funding table, so presenting it here compared a
    // 1,712-byte base against a 1,736-byte proved range and refused
    // `TokenRangeMismatch` -- by pointer identity, which no equality test
    // recovers. That made this conjunct UNSATISFIABLE for every capability
    // whose account profile is schema V3, and it went unnoticed because no V3
    // family had ever reached this statement: the Series arm is the first, and
    // it reached it only once the config-identity wall came down.
    sealed_ownership
        .require(
            selected_action,
            funding_profile.map_or_else(|| account_profile.bytes(), AccountProfileV3::bytes),
            lifecycle.bytes(),
            request_profile.bytes(),
            transition.bytes(),
        )
        .map_err(|error| {
            hot_cu_reason!("sealed-ownership", error);
            hot_cu_sealed_ownership_ranges!(
                sealed_ownership,
                selected_action,
                [
                    account_profile.bytes(),
                    lifecycle.bytes(),
                    request_profile.bytes(),
                    transition.bytes(),
                ]
            );
            TradingSbfError::Content
        })?;
    // Every register bank the rest of this execution needs is now already on
    // the heap. The projection rotated through three pairs and kept one; the
    // preplan arena takes the two it finished with, the interpreted transition
    // takes the request output once the preplan has copied it, and the replan
    // takes the preplan's own output once the candidate has consumed it. Under
    // an allocator whose `dealloc` is a no-op, each of those rentals is a whole
    // pair of `scalar_count` and `identity_count` banks that is never charged.
    let mut preplan_scratch = LifecyclePreplanScratchV4::new(
        &scratch,
        lifecycle_observations,
        lifecycle_runtime_accounts,
        scalar_count,
        identity_count,
        spare_scalars,
        spare_identities,
    )?;
    // The one pair this phase genuinely has to allocate: the preplan's input is
    // the request output and the arena holds the other two, so nothing dead is
    // available to rent yet. It is handed to the replan later rather than
    // dropped.
    hot_heap_mark!("preplan-arena");
    let preplan_output_scalars = ScratchVecV1::filled(&scratch, &0_u64, scalar_count)?;
    let preplan_output_identities = ScratchVecV1::filled(&scratch, &[0_u8; 32], identity_count)?;
    hot_heap_mark!("preplan-output");
    hot_cu_checkpoint!("p5-sealed-ownership-arena");
    // Destructured at the call, for the reason the request projection above
    // is: both register banks borrow the scratch region.
    let PreparedLifecycleBatchV4 {
        plans: preplanned_plans,
        scalars: replan_output_scalars,
        identities: replan_output_identities,
    } = prepare_lifecycle_v4(
        program_id,
        frame.registry,
        envelope.market(),
        envelope.release_set(),
        envelope.generation(),
        market.rent_beneficiary.to_bytes(),
        lifecycle,
        selected_action,
        account_profile,
        tail_count,
        lifecycle_observations,
        lifecycle_runtime_accounts,
        &request_output_scalars,
        &request_output_identities,
        &rent,
        lifecycle_aliases,
        profile_join,
        envelope.bump_hints().lifecycle,
        None,
        &mut preplan_scratch,
        preplan_output_scalars,
        preplan_output_identities,
    )?;
    hot_cu_checkpoint!("request-lifecycle-preplan");

    let candidate = if let Some(caller_authorities) = admitted_caller_authorities {
        // The transport binding is joined above, where the bank's own widths
        // are computed and the frame's two inputs are still live. See
        // `require_admitted_bank_matches_frame_v3` for why it is not joined
        // here.
        execute_admitted_candidate_v3(AdmittedCandidateViewV3 {
            program_id,
            frame,
            hot_fixed_accounts: accounts
                .get(..HOT_FIXED_ACCOUNT_COUNT_V3)
                .ok_or(TradingSbfError::Content)?,
            caller_authorities,
            output_page: admitted_output_page,
            strategy_extras,
            runtime_accounts: &runtime_accounts,
            input_scratch_pages,
            observations: &observations,
            envelope,
            context,
            descriptor,
            strategy,
            product_runtime_v3,
            family_request,
            root_prestate,
            selected_program,
            selected_action,
            tail_count,
            scalars: &replan_output_scalars,
            identities: &replan_output_identities,
            representatives: &aliases,
        })?
    } else {
        // The fold's OUTPUT pair is the one register bank of this execution
        // that survives the scratch release: the commit reads it, the child
        // walk reads it, and the effect projection is derived from it. So it
        // is the one pair allocated at the upward end, and it is allocated
        // fresh here rather than moved in from the request projection.
        //
        // W2n moved the dead request-projection pair in instead, to avoid
        // asking an allocator that never frees for a pair it had already
        // handed out. On an allocator that now DOES give the scratch end back
        // that trade inverts: reusing the request pair would pin the whole
        // three-pair projection, the preplan output pair and the arena --
        // 7,219 bytes on the canonical Direct bundle -- for the rest of the
        // instruction, to save allocating 1,600. The fold's scratch is still
        // rented from the preplan arena, which is idle between its two passes.
        execute_interpreted_transition_v3(
            transition,
            tail_count,
            TransitionRegistersV3 {
                input_scalars: &replan_output_scalars,
                input_identities: &replan_output_identities,
                scratch_scalars: &mut preplan_scratch.next_scalars,
                scratch_identities: &mut preplan_scratch.next_identities,
                output_scalars: try_projection_bank_v3(&0_u64, scalar_count)?,
                output_identities: try_projection_bank_v3(&[0_u8; 32], identity_count)?,
            },
        )?
    };
    hot_cu_checkpoint!("candidate");
    // The preplan's own output banks are dead the moment the candidate has
    // consumed them, and the replan needs exactly one pair of that width. It
    // rents these; only `plans` is still read after this point, by the replan
    // agreement.
    let transition_output_scalars = candidate.scalars;
    let transition_output_identities = candidate.identities;
    let admitted_execution_digest = candidate.transcript_digest;
    lifecycle
        .validate_projected_current_rent_quotes(
            account_profile,
            Some(profile_join),
            tail_count,
            selected_action,
            &transition_output_scalars,
            &current_rent_quotes,
        )
        .map_err(|_| TradingSbfError::Content)?;
    require_trusted_environment_v3(
        trusted_environment,
        &transition_output_scalars,
        &transition_output_identities,
    )?;
    require_dynamic_span_values_v3(
        account_profile,
        &dynamic_spans.widths,
        &transition_output_scalars,
    )?;
    require_funding_runtime_v5(
        program_id,
        frame.registry,
        effect,
        funding_profile,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        &runtime_accounts,
        &aliases,
        &rent,
        envelope.market(),
        envelope.release_set(),
        envelope.generation(),
        market.rent_beneficiary.to_bytes(),
    )?;
    hot_cu_checkpoint!("p7-post-candidate-checks");

    require_borrowed_witness_coverage_v3(
        request_profile,
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        family_request,
    )?;
    hot_cu_checkpoint!("p7-borrowed-witness");

    // The replan runs BEFORE the effect projection, and the order is
    // load-bearing rather than cosmetic. These two are independent -- each
    // reads the same preplanned table and the same transition outputs, and
    // neither reads anything the other writes -- but the replan is the LAST
    // reader of the observation bank, and the effect projection is where the
    // heap is deepest. Running the plan agreement first lets the scratch
    // region close before that depth is reached, which is what keeps the
    // intermediate high-water under the 32 KiB ceiling rather than merely the
    // final one. It is also the more conservative order: the transition's
    // outputs are held to the plan they were given before any effect is
    // projected from them.
    let PreparedLifecycleBatchV4 {
        plans: _replan_plans,
        scalars: revalidated_scalars,
        identities: revalidated_identities,
    } = prepare_lifecycle_v4(
        program_id,
        frame.registry,
        envelope.market(),
        envelope.release_set(),
        envelope.generation(),
        market.rent_beneficiary.to_bytes(),
        lifecycle,
        selected_action,
        account_profile,
        tail_count,
        lifecycle_observations,
        lifecycle_runtime_accounts,
        &transition_output_scalars,
        &transition_output_identities,
        &rent,
        lifecycle_aliases,
        profile_join,
        envelope.bump_hints().lifecycle,
        Some(&preplanned_plans),
        &mut preplan_scratch,
        replan_output_scalars,
        replan_output_identities,
    )?;
    hot_cu_checkpoint!("p7-replan");
    require_lifecycle_replan_agreement_v4(
        &revalidated_scalars,
        &revalidated_identities,
        &transition_output_scalars,
        &transition_output_identities,
    )?;
    hot_cu_checkpoint!("effect-lifecycle-replan");
    // The replan agreed with this table invocation by invocation rather than
    // building a duplicate of it, so the table the commit executes is the one
    // the transition was handed and the replan reproduced.
    let lifecycle_plans = preplanned_plans;
    let root_lifecycle_close = selected_root_lifecycle_close_v3(&lifecycle_plans)?;

    // What the effect projection reads out of the observation bank is two
    // numbers per coordinate, and it read them by walking the bank itself.
    // Taking them here instead is what frees the bank: sixteen bytes per
    // coordinate survive the release in place of forty-eight plus a borrow
    // guard.
    let account_inputs = account_inputs_v3(&observations)?;
    // The shadow strategy's only reading of the bank is a digest OF it, and
    // the digest is taken here for the same reason.
    let shadow_runtime_digest = if shadow_caller_authority.is_some() {
        Some(runtime_transcript_digest_v3(
            &observations,
            &runtime_accounts,
            &[],
        )?)
    } else {
        None
    };
    // Every reader of the scratch region has run. The list is exhaustive by
    // construction rather than by inspection: each of these borrows `scratch`,
    // so the borrow checker refuses this `drop(scratch)` while any one of them
    // is still in scope. The whole high end goes back in one store -- 13,043
    // bytes on the canonical Direct bundle -- before the effect projection,
    // the child walk and the commit build their own.
    drop(observations);
    drop(runtime_data);
    drop(request_output_scalars);
    drop(request_output_identities);
    drop(preplan_scratch);
    drop(revalidated_scalars);
    drop(revalidated_identities);
    drop(scratch);
    hot_cu_checkpoint!("observations-released");

    let projected_effects = project_hot_effects_v3(
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        account_inputs,
        &lifecycle_plans,
        &effect_permissions,
        &aliases,
        runtime_accounts.len(),
        request_bytes,
    )?;
    let output_lamports = projected_effects.lamports;
    let output_requests = projected_effects.requests;
    // The local-effect discipline is folded into the projection walk above, so
    // this checkpoint now brackets nothing: it is kept so the phase table stays
    // comparable across the lanes that measured the separate walk.
    let mut participation = projected_effects.participation;
    hot_cu_checkpoint!("p7-effect-projection");
    hot_cu_checkpoint!("p7-local-effect-discipline");
    if root_lifecycle_close {
        require_root_lifecycle_close_child_separation_v3(
            effect,
            tail_count,
            &transition_output_scalars,
            &transition_output_identities,
            &aliases,
        )?;
    }
    hot_cu_checkpoint!("pf-close-separation");
    // One byte per logical coordinate, decoded once, whole frame checked here.
    let effect_privileges = child_route_privileges_v3(
        account_profile,
        tail_count,
        &dynamic_spans.widths,
        &runtime_accounts,
    )?;
    hot_cu_checkpoint!("pf-child-privileges");
    let effect_accounts = downgraded_effect_accounts_v3(&runtime_accounts, &effect_privileges)?;
    hot_heap_mark!("downgraded-effects");
    hot_cu_checkpoint!("pf-downgraded-effects");
    // Resolved ONCE for the preflight walk and the execution walk both. See
    // `ChildWalkResolutionV3` for what each walk was paying to re-derive.
    let child_walk = resolve_child_walk_v3(
        *frame,
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        effect_accounts,
        &aliases,
        &output_requests,
        family_request,
        request_digest,
        envelope,
        root.child_programs,
    )?;
    hot_cu_checkpoint!("pf-composition");
    let caller_bumps = preflight_child_routes_v3(
        program_id,
        *frame,
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        effect_accounts,
        &output_requests,
        family_request,
        request_digest,
        envelope,
        context.selection().capability_release().to_bytes(),
        selected_program.to_bytes(),
        &aliases,
        &child_walk,
        participation.as_deref_mut(),
        authenticated_series_expiry_replay,
        authenticated_series_expiry_rent_credit,
    )?;
    hot_cu_checkpoint!("preflight-children");
    let series_expiry_replay_prestate = if authenticated_series_expiry_replay {
        let replay_root = runtime_accounts
            .first()
            .copied()
            .ok_or(TradingSbfError::Content)?;
        let replay_ticket = runtime_accounts
            .get(series_expiry::SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1)
            .copied()
            .ok_or(TradingSbfError::Content)?;
        let ticket_digest = hash(
            &replay_ticket
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Content)?,
        )
        .to_bytes();
        Some(SeriesExpiryReplayPrestateV1::authenticated(
            replay_root,
            root_prestate,
            replay_ticket,
            ticket_digest,
        )?)
    } else {
        None
    };
    let strategy_execution_digest = if let Some(caller_authority) = shadow_caller_authority {
        execute_shadow_candidate_v3(ShadowCandidateViewV3 {
            program_id,
            frame,
            caller_authority,
            strategy_extras,
            runtime_accounts: &runtime_accounts,
            runtime_observations_digest: shadow_runtime_digest.ok_or(TradingSbfError::Content)?,
            envelope,
            descriptor,
            strategy,
            family_request,
            root_prestate,
            selected_program,
            selected_action,
            effect,
            tail_count,
            scalars: &transition_output_scalars,
            identities: &transition_output_identities,
            output_lamports: &output_lamports,
            request_bank: &output_requests,
        })?
    } else {
        admitted_execution_digest
    };
    let direct_crosscheck = prepare_direct_inline_hot_crosscheck_v3(
        program_id,
        selected_kind,
        selected_action,
        direct_config,
        family_request,
        request_digest,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        &runtime_accounts,
        &lifecycle_plans,
        effect,
        &output_requests,
        envelope,
        selected_program,
        immutable_root_header,
        root_prestate,
        strategy_execution_digest,
        descriptor,
        strategy,
        context,
        market,
        product_runtime_v3,
        product_outcome_count,
        root.child_programs,
    )?;
    hot_cu_checkpoint!("children-shadow");
    let root_commit_plan = RootCommitPlanV3::for_geometry(effect, tail_count)?;
    hot_cu_checkpoint!("before-commit");
    let commit_status = commit_prepared_hot_v3(
        &caller_bumps,
        Box::new(PreparedHotCommitV3 {
            program_id,
            frame,
            request_profile,
            effect,
            tail_count,
            scalars: &transition_output_scalars,
            identities: &transition_output_identities,
            runtime_accounts: &runtime_accounts,
            effect_accounts,
            request_bank: &output_requests,
            family_request,
            request_digest,
            envelope,
            selected_program,
            funding_profile,
            lifecycle_plans: &lifecycle_plans,
            participation: participation.as_deref(),
            root_lifecycle_close,
            aliases: &aliases,
            output_lamports: &output_lamports,
            immutable_root_header,
            root_prestate,
            strategy_execution_digest,
            descriptor,
            strategy,
            context,
            market,
            product_runtime_v3,
            product_outcome_count,
            root_commit_plan,
            child_walk: &child_walk,
            direct_crosscheck,
            series_expiry_replay_prestate,
        }),
    );
    hot_cu_checkpoint!("after-commit");
    if commit_status == 0 {
        Ok(())
    } else {
        Err(ProgramError::from(commit_status))
    }
}

pub(super) struct PreparedHotCommitV3<'a, 'accounts, 'info, 'artifact> {
    program_id: &'a Pubkey,
    pub(super) frame: &'a HotFrameV3<'accounts, 'info>,
    request_profile: RequestProfileKindV3<'artifact>,
    effect: SelectedEffectProgramV4<'artifact>,
    tail_count: u32,
    scalars: &'a [u64],
    identities: &'a [[u8; 32]],
    pub(super) runtime_accounts: &'a [&'accounts AccountInfo<'info>],
    effect_accounts: DowngradedEffectAccountsV3<'a, 'accounts, 'info>,
    request_bank: &'a [u8],
    pub(super) family_request: &'a [u8],
    request_digest: [u8; 32],
    envelope: HotExecutionEnvelopeV3,
    selected_program: ContentId,
    funding_profile: Option<AccountProfileV3<'artifact>>,
    lifecycle_plans: &'a [PreparedLifecycleInvocationV3],
    root_lifecycle_close: bool,
    aliases: &'a [usize],
    output_lamports: &'a [u64],
    // Sixteen bytes on the boxed input, and they buy the one thing the lamport
    // plan cannot derive for itself: which coordinates a declared child route
    // reached. `None` when the Effect declares no child route at all, which is
    // exactly when the plan IS the sole authority.
    participation: Option<&'a [CoordinateParticipationV3]>,
    immutable_root_header: &'a [u8; CAPABILITY_ROOT_HEADER_BYTES_V1],
    pub(super) root_prestate: [u8; 32],
    strategy_execution_digest: [u8; 32],
    descriptor: &'a CapabilityProgramV4,
    strategy: &'a AuthenticatedExecutionStrategyV2,
    context: &'a TradingFamilyContextV1,
    market: &'a AuthenticatedLogicalMarketV3,
    product_runtime_v3: &'a AuthenticatedProductRuntimeV3<'accounts, 'info>,
    product_outcome_count: u32,
    // Allocated while the heap still has room. The commit phase only fills the
    // exact geometry-sized bitset; it never asks the non-reclaiming allocator
    // for its first block after child CPI.
    root_commit_plan: RootCommitPlanV3,
    // What the preflight walk already resolved out of the same Effect, the same
    // registers and the same activation cache; see `ChildWalkResolutionV3`.
    child_walk: &'a ChildWalkResolutionV3<'a, 'info>,
    pub(super) direct_crosscheck: Option<HeapBoxV3<DirectHotCrosscheckV3>>,
    pub(super) series_expiry_replay_prestate: Option<SeriesExpiryReplayPrestateV1>,
}

#[inline(never)]
fn commit_prepared_hot_v3(
    // Beside the boxed plan, not inside it: `PreparedHotCommitV3` is allocated
    // at `before-commit` and is live across the heap peak, so one more borrowed
    // register bank in it is eight more bytes the run does not have. See
    // `ChildCallerBumpsV4`.
    caller_bumps: &ChildCallerBumpsV4,
    mut prepared: Box<PreparedHotCommitV3<'_, '_, '_, '_>>,
) -> u64 {
    match commit_prepared_hot_result_v3(caller_bumps, &mut prepared) {
        Ok(()) => 0,
        Err(error) => error.into(),
    }
}

/// Keep the wide `ProgramError` return ABI inside the compact commit phase.
/// The outer verifier frame receives one scalar status register instead of an
/// indirect result slot that aliases its last live stack region.
#[inline(never)]
fn commit_prepared_hot_result_v3(
    caller_bumps: &ChildCallerBumpsV4,
    prepared: &mut PreparedHotCommitV3<'_, '_, '_, '_>,
) -> Result<(), ProgramError> {
    apply_lifecycle_creates_v3(
        prepared.program_id,
        prepared.lifecycle_plans,
        prepared.runtime_accounts,
    )?;
    // Between the two creates, because they were one bracket and both can
    // refuse `Commit`. The commit phase returns a STATUS rather than erroring
    // out of a `?`, so `after-commit` prints even on a refusal and the whole of
    // `commit_prepared_hot_result_v3` reads as one step from outside. The first
    // mark inside it was `lifecycle-creates`, AFTER both -- so a refusal in
    // either was indistinguishable, and 0x4005 has 237 sites in this program.
    // Splitting them costs one log line on a diagnostic build and turns
    // twelve-or-twenty-one into twelve or twenty-one.
    hot_heap_mark!("commit-lifecycle-creates");
    apply_funding_creates_v5(
        prepared.program_id,
        prepared.effect.funding(),
        prepared.tail_count,
        prepared.scalars,
        prepared.identities,
        prepared.runtime_accounts,
    )?;
    hot_heap_mark!("lifecycle-creates");
    let child_execution_digest = execute_prepared_child_routes_v3(caller_bumps, prepared)?;
    hot_heap_mark!("children-executed");
    verify_fractional_root_unchanged_after_children_v3(prepared)?;
    verify_series_expiry_replay_unchanged_after_children_v1(prepared)?;
    verify_direct_inline_post_children_v3(prepared)?;
    commit_prepared_post_children_v3(prepared)?;
    hot_heap_mark!("post-children");
    let root_poststate = if prepared.root_lifecycle_close {
        if prepared.frame.root.owner != &system_program::ID
            || prepared.frame.root.data_len() != 0
            || prepared.frame.root.lamports() != 0
        {
            return Err(TradingSbfError::Commit.into());
        }
        vacant_root_poststate_digest_v3(prepared.frame.root.key)
    } else {
        let bytes = prepared
            .frame
            .root
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Commit)?;
        if bytes.get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            != Some(prepared.immutable_root_header.as_slice())
        {
            return Err(TradingSbfError::Commit.into());
        }
        hash(&bytes).to_bytes()
    };
    finalize_hot_ack_v3(prepared, child_execution_digest, root_poststate)
}

#[inline(never)]
fn execute_prepared_child_routes_v3(
    caller_bumps: &ChildCallerBumpsV4,
    prepared: &PreparedHotCommitV3<'_, '_, '_, '_>,
) -> Result<[u8; 32], ProgramError> {
    execute_child_routes_v3(
        prepared.program_id,
        *prepared.frame,
        prepared.request_profile,
        prepared.effect,
        prepared.tail_count,
        prepared.scalars,
        prepared.identities,
        prepared.effect_accounts,
        prepared.aliases,
        prepared.request_bank,
        prepared.family_request,
        prepared.request_digest,
        prepared.envelope,
        prepared.context.selection().capability_release().to_bytes(),
        prepared.selected_program.to_bytes(),
        prepared.child_walk,
        caller_bumps,
        // Deferring a Claims child's post-resource verification to the Direct
        // finalization is only sound when there IS one, and the registered
        // creation variant has none: its planner re-derives three Trading-owned
        // accounts and no economic candidate. It also routes to Claims never --
        // a Sell escrows through the record it writes, a Buy through Custody --
        // so `Immediate` is not a weakening here, it is the only reading with a
        // subject.
        match prepared.direct_crosscheck.as_deref() {
            Some(DirectHotCrosscheckV3::InlineOrdinary { .. }) => {
                SparsePostResourceVerificationV3::DirectFinalization
            }
            Some(DirectHotCrosscheckV3::RegisteredCreation(_)) | None => {
                SparsePostResourceVerificationV3::Immediate
            }
        },
    )
}

#[inline(never)]
pub(super) fn commit_prepared_post_children_v3(
    prepared: &mut PreparedHotCommitV3<'_, '_, '_, '_>,
) -> Result<(), ProgramError> {
    apply_lifecycle_closes_v3(
        prepared.program_id,
        prepared.frame.registry,
        prepared.envelope.market(),
        prepared.envelope.release_set(),
        prepared.envelope.generation(),
        prepared.market.rent_beneficiary.to_bytes(),
        prepared.lifecycle_plans,
        prepared.runtime_accounts,
    )?;
    apply_funding_closes_v5(
        prepared.program_id,
        prepared.frame.registry,
        prepared.effect.funding(),
        prepared.funding_profile,
        prepared.tail_count,
        prepared.scalars,
        prepared.identities,
        prepared.runtime_accounts,
        prepared.envelope.market(),
        prepared.envelope.release_set(),
        prepared.envelope.generation(),
        prepared.market.rent_beneficiary.to_bytes(),
    )?;
    hot_cu_checkpoint!("commit-lifecycle-closes");
    commit_non_root_effects_into_v3(
        prepared.effect,
        prepared.tail_count,
        prepared.scalars,
        prepared.identities,
        prepared.runtime_accounts,
        prepared.aliases,
        prepared.output_lamports,
        prepared.participation,
        &mut prepared.root_commit_plan,
    )?;
    hot_cu_checkpoint!("commit-non-root");
    if prepared.root_lifecycle_close {
        if prepared.root_commit_plan.bits.iter().any(|byte| *byte != 0) {
            return Err(TradingSbfError::Commit.into());
        }
    } else {
        commit_root_effects_v3(
            prepared.effect,
            prepared.tail_count,
            prepared.scalars,
            prepared.identities,
            prepared.runtime_accounts,
            prepared.aliases,
            prepared.output_lamports,
            prepared.participation,
            &prepared.root_commit_plan,
        )?;
    }
    hot_cu_checkpoint!("commit-root");
    verify_direct_inline_local_poststate_v3(prepared)?;
    Ok(())
}

pub(super) fn vacant_root_poststate_digest_v3(root: &Pubkey) -> [u8; 32] {
    let lamports = 0_u64.to_le_bytes();
    let data_len = 0_u64.to_le_bytes();
    hashv(&[
        VACANT_ROOT_POSTSTATE_DOMAIN_V3,
        root.as_ref(),
        system_program::ID.as_ref(),
        &lamports,
        &data_len,
    ])
    .to_bytes()
}

#[inline(never)]
fn finalize_hot_ack_v3(
    prepared: &PreparedHotCommitV3<'_, '_, '_, '_>,
    child_execution_digest: [u8; 32],
    root_poststate: [u8; 32],
) -> Result<(), ProgramError> {
    let ack = project_hot_execution_ack_v3(
        HotExecutionAckInputV3 {
            release_set: prepared.envelope.release_set(),
            market: prepared.envelope.market(),
            generation: prepared.envelope.generation(),
            root: prepared.frame.root.key.to_bytes(),
            request_digest: prepared.request_digest,
            root_prestate_digest: prepared.root_prestate,
            artifacts: HotExecutionArtifactFactsV3 {
                selected_program: prepared.selected_program.to_bytes(),
                account_profile_program: prepared.descriptor.account_profile().program().to_bytes(),
                request_profile_program: prepared.descriptor.request_profile().program().to_bytes(),
                strategy_program: prepared.strategy.strategy_program_id().to_bytes(),
                strategy_transition_program: prepared
                    .strategy
                    .strategy()
                    .transition_program()
                    .to_bytes(),
                effect_program: prepared.descriptor.effect().program().to_bytes(),
                derivation_policy: prepared.descriptor.derivation_policy().to_bytes(),
                config: prepared.context.selection().config().to_bytes(),
                product_record: prepared.market.identity.product_record.to_bytes(),
                linked_basis_record_digest: prepared
                    .product_runtime_v3
                    .linked_basis_record
                    .content_digest
                    .to_bytes(),
                semantic_basis_id: prepared.product_runtime_v3.semantic_basis_id.to_bytes(),
                outcome_count: prepared.product_outcome_count,
                strategy_execution_digest: prepared.strategy_execution_digest,
            },
        },
        child_execution_digest,
        root_poststate,
    )
    .map_err(|_| TradingSbfError::Commit)?;
    // The ack and the ordered child transcript are the INLINE planner's second
    // opinion, and only its. A registered creation's children are Custody's
    // three, whose receipts it cannot predict without reimplementing Custody --
    // so it does not claim to, and this comparison does not run for it. What the
    // registered variant asserts is stated on `DirectHotCrosscheckV3` and
    // checked in `verify_direct_inline_local_poststate_v3`.
    if let Some(DirectHotCrosscheckV3::InlineOrdinary { finalization, .. }) =
        prepared.direct_crosscheck.as_deref()
    {
        if child_execution_digest
            != finalization
                .child_execution_digest()
                .map_err(|_| TradingSbfError::Commit)?
            || root_poststate
                != finalization
                    .poststate(0)
                    .map_err(|_| TradingSbfError::Commit)?
                    .data_digest
            || ack != finalization.ack().map_err(|_| TradingSbfError::Commit)?
            || ack.to_bytes()
                != *finalization
                    .ack_bytes()
                    .map_err(|_| TradingSbfError::Commit)?
        {
            return Err(TradingSbfError::Commit.into());
        }
    }
    set_return_data(&ack.to_bytes());
    Ok(())
}
