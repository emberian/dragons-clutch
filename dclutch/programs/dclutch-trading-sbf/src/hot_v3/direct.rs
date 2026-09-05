//! The Direct family's second opinion: an independently prepared inline fill
//! (and registered creation) that the commit must agree with byte for byte.

use super::*;

#[inline(never)]
pub(super) fn verify_direct_inline_post_children_v3(
    prepared: &PreparedHotCommitV3<'_, '_, '_, '_>,
) -> Result<(), ProgramError> {
    let Some(crosscheck) = prepared.direct_crosscheck.as_ref() else {
        return Ok(());
    };
    match &**crosscheck {
        DirectHotCrosscheckV3::InlineOrdinary {
            poststate_accounts,
            finalization,
        } => {
            for (coordinate, role_index) in [
                (poststate_accounts.claims_market, 3_usize),
                (poststate_accounts.seller_position, 4_usize),
                (poststate_accounts.buyer_position, 5_usize),
                (poststate_accounts.custody_replay, 6_usize),
                (poststate_accounts.buyer_token, 7_usize),
                (poststate_accounts.seller_token, 8_usize),
                (poststate_accounts.fee_token, 9_usize),
            ] {
                let account = direct_runtime_account_v3(prepared.runtime_accounts, coordinate)?;
                let expected = finalization
                    .poststate(role_index)
                    .map_err(|_| TradingSbfError::Commit)?;
                verify_direct_inline_account_poststate_v3(account, expected)?;
            }
        }
        // A Sell's array is empty and this loop runs zero times, which is the
        // correct reading rather than an omission: a Sell escrows claims through
        // the record it just wrote and opens no Custody frame, so there is no
        // child account for a second opinion to hold one about.
        DirectHotCrosscheckV3::RegisteredCreation(registered) => {
            for child in registered.children.iter().flatten() {
                let account =
                    direct_runtime_account_v3(prepared.runtime_accounts, child.coordinate)?;
                verify_direct_registered_child_account_v3(account, *child)?;
            }
        }
    }
    Ok(())
}

/// Verify the three facts Direct quotes about an account its CHILD creates.
///
/// Owner, funded balance and width, and deliberately not the bytes. See
/// `DirectRegisteredChildAccountV3`: the body is Custody's, and
/// `verify_custody_receipt_v3` has already bound it to Custody's own receipt on
/// this same path.
fn verify_direct_registered_child_account_v3(
    account: &AccountInfo<'_>,
    expected: DirectRegisteredChildAccountV3,
) -> Result<(), ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Commit)?;
    if account.owner.to_bytes() != expected.owner
        || account.lamports() != expected.lamports
        || u32::try_from(data.len()).map_err(|_| TradingSbfError::Commit)? != expected.data_len
    {
        return Err(TradingSbfError::Commit.into());
    }
    Ok(())
}

/// Verify one account the registered Direct planner re-derived in full.
fn verify_direct_registered_poststate_v3(
    account: &AccountInfo<'_>,
    expected: DirectRegisteredPoststateV3,
) -> Result<(), ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Commit)?;
    if account.key.to_bytes() != expected.address
        || account.owner.to_bytes() != expected.owner
        || account.lamports() != expected.lamports
        || u32::try_from(data.len()).map_err(|_| TradingSbfError::Commit)? != expected.data_len
        || hash(&data).to_bytes() != expected.data_digest
    {
        return Err(TradingSbfError::Commit.into());
    }
    Ok(())
}

#[inline(never)]
pub(super) fn verify_direct_inline_local_poststate_v3(
    prepared: &PreparedHotCommitV3<'_, '_, '_, '_>,
) -> Result<(), ProgramError> {
    let Some(crosscheck) = prepared.direct_crosscheck.as_ref() else {
        return Ok(());
    };
    match &**crosscheck {
        DirectHotCrosscheckV3::InlineOrdinary { finalization, .. } => {
            for (coordinate, role_index) in [(0_usize, 0_usize), (5, 1), (8, 2)] {
                let account = prepared
                    .runtime_accounts
                    .get(coordinate)
                    .ok_or(TradingSbfError::Commit)?;
                let expected = finalization
                    .poststate(role_index)
                    .map_err(|_| TradingSbfError::Commit)?;
                verify_direct_inline_account_poststate_v3(account, expected)?;
            }
        }
        DirectHotCrosscheckV3::RegisteredCreation(registered) => {
            for expected in registered.poststates {
                let account =
                    direct_runtime_account_v3(prepared.runtime_accounts, expected.coordinate)?;
                verify_direct_registered_poststate_v3(account, expected)?;
            }
        }
    }
    Ok(())
}

fn verify_direct_inline_account_poststate_v3(
    account: &AccountInfo<'_>,
    expected: &DirectInlinePoststateCommitmentV3,
) -> Result<(), ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Commit)?;
    if account.key.to_bytes() != expected.address
        || account.owner.to_bytes() != expected.owner
        || account.lamports() != expected.lamports
        || u32::try_from(data.len()).map_err(|_| TradingSbfError::Commit)? != expected.data_len
        || hash(&data).to_bytes() != expected.data_digest
    {
        return Err(TradingSbfError::Commit.into());
    }
    Ok(())
}

/// Direct-only semantic cross-check retained through child execution.
///
/// This value has no dispatch authority. The authenticated Effect continues
/// to own the request bank and child walk; this stores the independently
/// prepared typed candidate so post-CPI checks can join the same facts.
/// The Direct planner's opinion about one execution, against the effect kernel's.
///
/// A crosscheck that cannot check an action must REFUSE it -- that is why wall A
/// stood for every registered action while this had one variant. It is not a
/// gate to relax; the second opinion either exists or it does not.
///
/// The two variants assert different things because the planner knows different
/// things, and saying so is the point. The inline ordinary route's planner
/// re-derives an economic candidate, ten account poststates, the ordered child
/// transcript and the acknowledgement, because every child it invokes is one
/// whose result it can predict. Registered creation's planner re-derives the
/// three accounts DIRECT owns -- the root, the maker replay and the registered
/// record -- and does not pretend to the rest.
pub(super) enum DirectHotCrosscheckV3 {
    /// Immediate ordinary match: candidate, ten poststates, transcript, ack.
    InlineOrdinary {
        poststate_accounts: DirectInlinePoststateAccountsV3,
        finalization: HeapBoxV3<DirectInlineFinalizationWorkspaceV3>,
    },
    /// `RegisterSell` or `RegisterBuy`: the three Direct-owned accounts, and for
    /// a Buy the two accounts Custody creates.
    RegisteredCreation(HeapBoxV3<DirectRegisteredCrosscheckV3>),
}

/// Which Direct actions have a planner that reads the immutable config.
///
/// ONE list, because two would drift and the drift is silent: the decode site
/// above hands `None` to a planner that needs a config, and the planner refuses
/// `Content` from a line that reads like a malformed request. Measured exactly
/// that way on 2026-09-01 -- the registered Sell crossed wall A and then died at
/// 330,040 CU on `direct_config.ok_or(..)`, because the decode was still
/// spelled `== InlineOrdinary` while the crosscheck had grown two more actions.
pub(super) const fn direct_action_crosschecks_against_config_v3(action: u32) -> bool {
    action == DirectExecutionActionV3::InlineOrdinary as u32
        || action == DirectExecutionActionV3::RegisterSell as u32
        || action == DirectExecutionActionV3::RegisterBuy as u32
}

/// Exact registered-creation poststate count: root, maker replay, record.
const DIRECT_REGISTERED_POSTSTATE_COUNT_V3: usize = 3;

// THE REGISTERED CREATION COORDINATES, AND WHY THEY ARE RESTATED HERE.
//
// `registered_{account,creation,state}_artifacts_v4` are
// `#[cfg(not(target_os = "solana"))]`, and correctly so: they EMIT artifacts,
// which is a build-time job, and this program CONSUMES them. So the on-chain
// crosscheck cannot import its coordinates from their author.
//
// Restating a coordinate is the defect class `68f7c849` spent a wall on, so
// every one below is pinned to that author by a host-only assertion. The
// program-test, `cargo check` and CI all build for the host, so a coordinate
// that drifts stops compiling there before it can reach a chain.
const DIRECT_REGISTERED_MAKER_ACCOUNT_V4: u16 = 5;
const DIRECT_REGISTERED_RECORD_ACCOUNT_V4: u16 = 8;
const DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4: u16 = 12;
const DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4: u16 = 25;
const REGISTERED_SCALAR_REPLAY_RENT_V4: usize = 50;
const REGISTERED_SCALAR_VAULT_RENT_V4: usize = 51;
const REGISTERED_IDENTITY_TOKEN_PROGRAM_V4: usize = 25;

#[cfg(not(target_os = "solana"))]
const _: () = {
    use dclutch_direct_codec::{
        registered_account_artifacts_v4 as accounts, registered_creation_artifacts_v4 as creation,
        registered_state_artifacts_v4 as state,
    };
    assert!(DIRECT_REGISTERED_MAKER_ACCOUNT_V4 == state::DIRECT_REGISTERED_MAKER_ACCOUNT_V4);
    assert!(DIRECT_REGISTERED_RECORD_ACCOUNT_V4 == state::DIRECT_REGISTERED_RECORD_ACCOUNT_V4);
    assert!(
        DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4
            == accounts::DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4
    );
    assert!(
        DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4
            == accounts::DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4
    );
    assert!(REGISTERED_SCALAR_REPLAY_RENT_V4 == creation::REGISTERED_SCALAR_REPLAY_RENT_V4);
    assert!(REGISTERED_SCALAR_VAULT_RENT_V4 == creation::REGISTERED_SCALAR_VAULT_RENT_V4);
    assert!(REGISTERED_IDENTITY_TOKEN_PROGRAM_V4 == creation::REGISTERED_IDENTITY_TOKEN_PROGRAM_V4);
};

/// One account the registered Direct planner re-derived IN FULL.
///
/// Every field is the planner's own, computed from `register_intent_v2` and the
/// artifacts, never read back off the account it will be compared against. A
/// commitment whose expectation came from its own subject is the guard-that-
/// compares-a-value-to-itself class this lane convicted twice on 2026-09-01.
#[derive(Clone, Copy)]
struct DirectRegisteredPoststateV3 {
    coordinate: u16,
    address: [u8; 32],
    owner: [u8; 32],
    lamports: u64,
    data_len: u32,
    data_digest: [u8; 32],
}

/// One account a CHILD creates, whose BODY is that child's to author.
///
/// The registered Buy opens a Custody replay and a TradingPrincipal vault. Their
/// owner, funded balance and width are facts Direct quotes and can therefore
/// check; their BYTES are Custody's, and Trading re-deriving them would make it
/// a second semantic owner of the replay encoder -- the thing the architecture
/// forbids, and the thing that already went wrong once today when this family's
/// profile wrote a Realm ADDRESS into a field holding a content digest.
///
/// The bytes are not unchecked. `execute_custody_route_v3` hashes the replay
/// after the CPI and `verify_custody_receipt_v3` binds that digest to the
/// receipt Custody itself returned, on this same path, before this runs.
#[derive(Clone, Copy)]
struct DirectRegisteredChildAccountV3 {
    coordinate: u16,
    owner: [u8; 32],
    lamports: u64,
    data_len: u32,
}

pub(super) struct DirectRegisteredCrosscheckV3 {
    poststates: [DirectRegisteredPoststateV3; DIRECT_REGISTERED_POSTSTATE_COUNT_V3],
    /// The Custody replay and vault, present for a Buy and absent for a Sell.
    ///
    /// A Sell escrows CLAIMS and opens no Custody frame at all, so there is
    /// nothing here rather than a zeroed pair -- an absent child cannot be
    /// checked into existence.
    children: [Option<DirectRegisteredChildAccountV3>; 2],
}

#[derive(Clone, Copy)]
pub(super) struct DirectInlinePoststateAccountsV3 {
    claims_market: u16,
    seller_position: u16,
    buyer_position: u16,
    buyer_token: u16,
    seller_token: u16,
    fee_token: u16,
    custody_replay: u16,
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn prepare_direct_inline_hot_crosscheck_v3(
    program_id: &Pubkey,
    selected_kind: [u8; 32],
    selected_action: u32,
    direct_config: Option<DirectExecutionConfigV1>,
    family_request: &[u8],
    request_digest: [u8; 32],
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    runtime_accounts: &[&AccountInfo<'_>],
    lifecycle_plans: &[PreparedLifecycleInvocationV3],
    effect: SelectedEffectProgramV4<'_>,
    request_bank: &[u8],
    envelope: HotExecutionEnvelopeV3,
    selected_program: ContentId,
    immutable_root_header: &[u8; CAPABILITY_ROOT_HEADER_BYTES_V1],
    root_prestate: [u8; 32],
    strategy_execution_digest: [u8; 32],
    descriptor: &CapabilityProgramV4,
    strategy: &AuthenticatedExecutionStrategyV2,
    family_context: &TradingFamilyContextV1,
    market: &AuthenticatedLogicalMarketV3,
    product_runtime_v3: &AuthenticatedProductRuntimeV3<'_, '_>,
    product_outcome_count: u32,
    child_programs: Option<AuthenticatedChildProgramsV3>,
) -> Result<Option<HeapBoxV3<DirectHotCrosscheckV3>>, ProgramError> {
    if selected_kind != DIRECT_SUCCESSOR_KIND_ID_V3 {
        return Ok(None);
    }
    let config = direct_config.ok_or(TradingSbfError::Content)?;
    // WALL A, and it was never a gate. This arm used to refuse every registered
    // action outright -- RegisterSell, RegisterBuy, the fills, the terminals,
    // both splits and both merges -- because a crosscheck that cannot check an
    // action must refuse it, and only the inline planner existed. The two
    // creation actions now have one, so they dispatch instead of refusing. Every
    // other registered action still refuses HERE, correctly and for the
    // unchanged reason, until its own planner is written.
    if direct_action_crosschecks_against_config_v3(selected_action)
        && selected_action != DirectExecutionActionV3::InlineOrdinary as u32
    {
        return Ok(Some(HeapBoxV3::new(
            DirectHotCrosscheckV3::RegisteredCreation(prepare_direct_registered_crosscheck_v3(
                program_id,
                selected_action,
                config,
                family_request,
                tail_count,
                scalars,
                identities,
                runtime_accounts,
                lifecycle_plans,
                immutable_root_header,
                child_programs,
            )?),
        )?));
    }
    if selected_action != DirectExecutionActionV3::InlineOrdinary as u32 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let request = match DirectExecutionRequestV3::decode(family_request, tail_count)
        .map_err(|_| TradingSbfError::Content)?
    {
        DirectExecutionRequestV3::InlineOrdinary(request) => request,
        _ => return Err(TradingSbfError::Content.into()),
    };
    let direct = prepare_direct_inline_input_v3(
        request,
        config,
        tail_count,
        scalars,
        runtime_accounts,
        lifecycle_plans,
    )?;
    let context =
        prepare_direct_inline_context_v3(tail_count, scalars, identities, request_digest)?;
    let (dispatch, poststate_accounts) =
        direct_inline_effect_dispatch_v3(effect, tail_count, scalars, identities)?;
    let collateral = prepare_direct_inline_collateral_v3(
        runtime_accounts,
        identities,
        *context,
        poststate_accounts,
    )?;
    let children = child_programs.ok_or(TradingSbfError::Release)?;
    let root_account = direct_runtime_account_v3(runtime_accounts, 0)?;
    let ack = HeapBoxV3::new(HotExecutionAckInputV3 {
        release_set: envelope.release_set(),
        market: envelope.market(),
        generation: envelope.generation(),
        root: root_account.key.to_bytes(),
        request_digest,
        root_prestate_digest: root_prestate,
        artifacts: HotExecutionArtifactFactsV3 {
            selected_program: selected_program.to_bytes(),
            account_profile_program: descriptor.account_profile().program().to_bytes(),
            request_profile_program: descriptor.request_profile().program().to_bytes(),
            strategy_program: strategy.strategy_program_id().to_bytes(),
            strategy_transition_program: strategy.strategy().transition_program().to_bytes(),
            effect_program: descriptor.effect().program().to_bytes(),
            derivation_policy: descriptor.derivation_policy().to_bytes(),
            config: family_context.selection().config().to_bytes(),
            product_record: market.identity.product_record.to_bytes(),
            linked_basis_record_digest: product_runtime_v3
                .linked_basis_record
                .content_digest
                .to_bytes(),
            semantic_basis_id: product_runtime_v3.semantic_basis_id.to_bytes(),
            outcome_count: product_outcome_count,
            strategy_execution_digest,
        },
    })?;
    let finalization = prepare_direct_inline_account_finalization_v3(
        program_id,
        runtime_accounts,
        poststate_accounts,
        &direct,
        &context,
        &collateral,
        dispatch,
        request_bank,
        family_request,
        children,
        immutable_root_header,
        market.identity.product_id.to_bytes(),
        &ack,
    )?;
    Ok(Some(HeapBoxV3::new(
        DirectHotCrosscheckV3::InlineOrdinary {
            poststate_accounts,
            finalization,
        },
    )?))
}

/// Re-derive the three accounts a registered creation writes, independently.
///
/// The second opinion is `successor::register_intent_v2`, which is the same
/// function the transition runs and is called here on inputs assembled from the
/// runtime frame rather than handed over -- the root off coordinate 0, the maker
/// replay observation and its first-use funding off the maker coordinate and its
/// lifecycle plan, the record's first-use funding off the record's. If the
/// planner and the effect kernel disagree about a single byte of the root, the
/// maker replay or the record, the commit refuses.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn prepare_direct_registered_crosscheck_v3(
    program_id: &Pubkey,
    selected_action: u32,
    config: DirectExecutionConfigV1,
    family_request: &[u8],
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    runtime_accounts: &[&AccountInfo<'_>],
    lifecycle_plans: &[PreparedLifecycleInvocationV3],
    immutable_root_header: &[u8; CAPABILITY_ROOT_HEADER_BYTES_V1],
    child_programs: Option<AuthenticatedChildProgramsV3>,
) -> Result<HeapBoxV3<DirectRegisteredCrosscheckV3>, ProgramError> {
    let buy = selected_action == DirectExecutionActionV3::RegisterBuy as u32;
    let request = match DirectExecutionRequestV3::decode(family_request, tail_count)
        .map_err(|_| TradingSbfError::Content)?
    {
        DirectExecutionRequestV3::RegisterSell(request) if !buy => request,
        DirectExecutionRequestV3::RegisterBuy(request) if buy => request,
        _ => return Err(TradingSbfError::Content.into()),
    };
    let maker_coordinate = usize::from(DIRECT_REGISTERED_MAKER_ACCOUNT_V4);
    let record_coordinate = usize::from(DIRECT_REGISTERED_RECORD_ACCOUNT_V4);

    let root_account = direct_runtime_account_v3(runtime_accounts, 0)?;
    let root_data = root_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let root_tail = root_data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(TradingSbfError::Content)?;
    if root_tail.len() != DIRECT_ROOT_STATE_BYTES_V1 {
        return Err(TradingSbfError::Content.into());
    }
    let root = DirectRootStateV1::decode(root_tail).map_err(|_| TradingSbfError::Content)?;
    let root_lamports = root_account.lamports();
    drop(root_data);

    // The maker replay's observation and first-use funding, read exactly the way
    // the inline route reads its two participants: the account decides vacant
    // from existing, and the lifecycle plan must agree.
    let maker = direct_inline_participant_v3(
        request.participant,
        maker_coordinate,
        runtime_accounts,
        lifecycle_plans,
    )?;

    let record_account =
        direct_runtime_account_v3(runtime_accounts, DIRECT_REGISTERED_RECORD_ACCOUNT_V4)?;
    let record_plan = lifecycle_plans
        .iter()
        .find(|plan| plan.state == record_coordinate)
        .ok_or(TradingSbfError::Content)?;
    let StateLifecyclePlanV3::Create(record_create) = record_plan.plan else {
        // A registered creation CREATES its record. An `Authenticate` plan here
        // is a second registration onto a live record, which the transition
        // refuses on its own terms and which this must not model.
        return Err(TradingSbfError::Content.into());
    };
    let created = register_intent_v2(
        root,
        maker.maker_replay,
        maker.authenticated,
        config,
        tail_count,
        maker.first_use,
        RegisteredRecordFirstUseV2 {
            bump: record_create.bump,
            observed_lamports: record_account.lamports(),
            rent_owner: record_create.beneficiary,
            rent_principal: record_create.historical_rent_principal,
        },
    )
    .map_err(|_| TradingSbfError::Transition)?;

    let trading = program_id.to_bytes();
    let mut root_bytes = Vec::new();
    root_bytes
        .try_reserve_exact(CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1)
        .map_err(|_| TradingSbfError::HeapExhausted)?;
    root_bytes.extend_from_slice(immutable_root_header);
    root_bytes.extend_from_slice(&created.root.encode());
    let maker_bytes = created
        .maker_root
        .encode()
        .map_err(|_| TradingSbfError::Transition)?;
    let record_bytes = created
        .record
        .encode_selected(config, tail_count)
        .map_err(|_| TradingSbfError::Transition)?;

    // The root is not funded by a creation -- the payer funds the replay and the
    // record -- so the planner's opinion of its balance is that it does not move.
    let poststates = [
        DirectRegisteredPoststateV3 {
            coordinate: 0,
            address: root_account.key.to_bytes(),
            owner: trading,
            lamports: root_lamports,
            data_len: width_u32_v3(root_bytes.len())?,
            data_digest: hash(&root_bytes).to_bytes(),
        },
        DirectRegisteredPoststateV3 {
            coordinate: DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            address: direct_runtime_account_v3(
                runtime_accounts,
                DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            )?
            .key
            .to_bytes(),
            owner: trading,
            lamports: match created.maker_creation {
                Some(plan) => plan.post_lamports,
                None => {
                    direct_runtime_account_v3(runtime_accounts, DIRECT_REGISTERED_MAKER_ACCOUNT_V4)?
                        .lamports()
                }
            },
            data_len: width_u32_v3(maker_bytes.len())?,
            data_digest: hash(&maker_bytes).to_bytes(),
        },
        DirectRegisteredPoststateV3 {
            coordinate: DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
            address: record_account.key.to_bytes(),
            owner: trading,
            lamports: created.record_creation.post_lamports,
            data_len: width_u32_v3(record_bytes.len())?,
            data_digest: hash(&record_bytes).to_bytes(),
        },
    ];

    let children = if buy {
        let custody = child_programs.ok_or(TradingSbfError::Release)?.custody;
        [
            Some(DirectRegisteredChildAccountV3 {
                coordinate: DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4
                    + CUSTODY_REPLAY_FRAME_COORDINATE_V1_U16,
                owner: custody,
                lamports: direct_scalar_v3(scalars, REGISTERED_SCALAR_REPLAY_RENT_V4)?,
                data_len: width_u32_v3(CUSTODY_REPLAY_BYTES_V1)?,
            }),
            // THE VAULT IS A TOKEN ACCOUNT, so the token program owns it and
            // Custody does not. Written expecting `custody` and the crosscheck
            // said otherwise on the first real Buy -- coordinate 35, lamports
            // and width exact, owner wrong -- which is the crosscheck doing the
            // job it exists for, against its own author.
            //
            // The expectation is the PROJECTED token-program identity, the same
            // register the Effect writes into `CustodyRequestLayoutV1::
            // TOKEN_PROGRAM`, and it is projected out of the Realm record.
            // Custody independently requires the live frame's token program to
            // equal that field and the mint's owner to be it, so the two
            // opinions meet without either reading the vault back off itself.
            Some(DirectRegisteredChildAccountV3 {
                coordinate: DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4 + OPEN_VAULT_FRAME_VAULT_V3,
                owner: direct_identity_v3(identities, REGISTERED_IDENTITY_TOKEN_PROGRAM_V4)?,
                lamports: direct_scalar_v3(scalars, REGISTERED_SCALAR_VAULT_RENT_V4)?,
                data_len: width_u32_v3(dclutch_token_svm::ACCOUNT_BYTES)?,
            }),
        ]
    } else {
        [None, None]
    };
    HeapBoxV3::new(DirectRegisteredCrosscheckV3 {
        poststates,
        children,
    })
}

/// The vault's coordinate inside a Custody `OpenVault` frame.
const OPEN_VAULT_FRAME_VAULT_V3: u16 = 10;
/// `CUSTODY_REPLAY_FRAME_COORDINATE_V1`, narrowed once and pinned to its author.
const CUSTODY_REPLAY_FRAME_COORDINATE_V1_U16: u16 = 8;
const _: () = {
    assert!(
        CUSTODY_REPLAY_FRAME_COORDINATE_V1_U16 as usize
            == crate::custody_composition_v3::CUSTODY_REPLAY_FRAME_COORDINATE_V1
    );
};

fn width_u32_v3(value: usize) -> Result<u32, ProgramError> {
    u32::try_from(value).map_err(|_| TradingSbfError::Content.into())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn prepare_direct_inline_account_finalization_v3(
    program_id: &Pubkey,
    runtime_accounts: &[&AccountInfo<'_>],
    poststate_accounts: DirectInlinePoststateAccountsV3,
    direct: &InlineOrdinaryInputV2,
    context: &DirectInlineCandidateContextV2,
    collateral: &DirectInlineCollateralFrameV2,
    dispatch: DirectInlineEffectDispatchV2,
    request_bank: &[u8],
    family_request: &[u8],
    children: AuthenticatedChildProgramsV3,
    immutable_root_header: &[u8; CAPABILITY_ROOT_HEADER_BYTES_V1],
    product_id: [u8; 32],
    ack: &HotExecutionAckInputV3,
) -> Result<HeapBoxV3<DirectInlineFinalizationWorkspaceV3>, ProgramError> {
    let root_account = direct_runtime_account_v3(runtime_accounts, 0)?;
    let seller_maker = direct_runtime_account_v3(runtime_accounts, 5)?;
    let buyer_maker = direct_runtime_account_v3(runtime_accounts, 8)?;
    let claims_market =
        direct_runtime_account_v3(runtime_accounts, poststate_accounts.claims_market)?;
    let seller_position =
        direct_runtime_account_v3(runtime_accounts, poststate_accounts.seller_position)?;
    let buyer_position =
        direct_runtime_account_v3(runtime_accounts, poststate_accounts.buyer_position)?;
    let custody_replay =
        direct_runtime_account_v3(runtime_accounts, poststate_accounts.custody_replay)?;
    let buyer_token = direct_runtime_account_v3(runtime_accounts, poststate_accounts.buyer_token)?;
    let seller_token =
        direct_runtime_account_v3(runtime_accounts, poststate_accounts.seller_token)?;
    let fee_token = direct_runtime_account_v3(runtime_accounts, poststate_accounts.fee_token)?;
    let mut account_data = Vec::new();
    account_data
        .try_reserve_exact(DIRECT_INLINE_POSTSTATE_COUNT_V3)
        .map_err(|_| TradingSbfError::HeapExhausted)?;
    for account in [
        root_account,
        seller_maker,
        buyer_maker,
        claims_market,
        seller_position,
        buyer_position,
        custody_replay,
        buyer_token,
        seller_token,
        fee_token,
    ] {
        account_data.push(
            account
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Content)?,
        );
    }
    let root_data = account_data.first().ok_or(TradingSbfError::Content)?;
    if root_data.get(..CAPABILITY_ROOT_HEADER_BYTES_V1) != Some(immutable_root_header.as_slice()) {
        return Err(TradingSbfError::Content.into());
    }
    let seller_maker_data = account_data.get(1).ok_or(TradingSbfError::Content)?;
    let buyer_maker_data = account_data.get(2).ok_or(TradingSbfError::Content)?;
    let claims_market_data = account_data.get(3).ok_or(TradingSbfError::Content)?;
    let seller_position_data = account_data.get(4).ok_or(TradingSbfError::Content)?;
    let buyer_position_data = account_data.get(5).ok_or(TradingSbfError::Content)?;
    let custody_replay_data = account_data.get(6).ok_or(TradingSbfError::Content)?;
    let buyer_token_data = account_data.get(7).ok_or(TradingSbfError::Content)?;
    let seller_token_data = account_data.get(8).ok_or(TradingSbfError::Content)?;
    let fee_token_data = account_data.get(9).ok_or(TradingSbfError::Content)?;
    let account_prestates = HeapBoxV3::new(DirectInlineAccountPrestatesV3 {
        root: direct_account_prestate_v3(root_account, root_data),
        seller_maker_replay: direct_account_prestate_v3(seller_maker, seller_maker_data),
        buyer_maker_replay: direct_account_prestate_v3(buyer_maker, buyer_maker_data),
        claims_market: direct_account_prestate_v3(claims_market, claims_market_data),
        seller_position: direct_account_prestate_v3(seller_position, seller_position_data),
        buyer_position: direct_account_prestate_v3(buyer_position, buyer_position_data),
        custody_replay: direct_account_prestate_v3(custody_replay, custody_replay_data),
        buyer_token: direct_account_prestate_v3(buyer_token, buyer_token_data),
        seller_token: direct_account_prestate_v3(seller_token, seller_token_data),
        fee_token: direct_account_prestate_v3(fee_token, fee_token_data),
    })?;
    let finalization_input = DirectInlineFinalizationInputV3 {
        direct,
        context,
        product_id,
        collateral,
        request_bank,
        dispatch,
        family_request,
        accounts: &account_prestates,
        programs: DirectInlineFinalizationProgramsV3 {
            trading: program_id.to_bytes(),
            claims: children.claims,
            custody: children.custody,
            token: context.token_program,
        },
        ack,
    };
    let mut finalization = HeapBoxV3::new(DirectInlineFinalizationWorkspaceV3::vacant())?;
    prepare_direct_inline_finalization_into_v3(&finalization_input, &mut finalization)
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(finalization)
}

fn direct_account_prestate_v3<'a>(
    account: &AccountInfo<'_>,
    data: &'a [u8],
) -> DirectInlineAccountPrestateV3<'a> {
    DirectInlineAccountPrestateV3 {
        address: account.key.to_bytes(),
        owner: account.owner.to_bytes(),
        lamports: account.lamports(),
        data,
    }
}

fn prepare_direct_inline_input_v3(
    request: dclutch_direct_codec::execution_v3::DirectInlineOrdinaryRequestV3,
    config: DirectExecutionConfigV1,
    tail_count: u32,
    scalars: &[u64],
    runtime_accounts: &[&AccountInfo<'_>],
    lifecycle_plans: &[PreparedLifecycleInvocationV3],
) -> Result<HeapBoxV3<InlineOrdinaryInputV2>, ProgramError> {
    let root_account = runtime_accounts.first().ok_or(TradingSbfError::Content)?;
    let root_data = root_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let root_tail = root_data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(TradingSbfError::Content)?;
    if root_tail.len() != DIRECT_ROOT_STATE_BYTES_V1 {
        return Err(TradingSbfError::Content.into());
    }
    let root = DirectRootStateV1::decode(root_tail).map_err(|_| TradingSbfError::Content)?;
    drop(root_data);
    let seller =
        direct_inline_participant_v3(request.seller, 5, runtime_accounts, lifecycle_plans)?;
    let buyer = direct_inline_participant_v3(request.buyer, 8, runtime_accounts, lifecycle_plans)?;
    HeapBoxV3::new(InlineOrdinaryInputV2 {
        root,
        seller,
        buyer,
        execution: InlineExecutionV2 {
            config,
            outcome_count: tail_count,
            slot: direct_scalar_v3(scalars, SCALAR_SLOT_V3)?,
            fill: request.fill,
            execution_price: request.execution_price,
        },
    })
}

fn prepare_direct_inline_context_v3(
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    request_digest: [u8; 32],
) -> Result<HeapBoxV3<DirectInlineCandidateContextV2>, ProgramError> {
    HeapBoxV3::new(DirectInlineCandidateContextV2 {
        release_set: direct_identity_v3(identities, IDENTITY_RELEASE_SET_V3)?,
        market: direct_identity_v3(identities, IDENTITY_MARKET_V3)?,
        generation: direct_scalar_v3(scalars, SCALAR_MARKET_GENERATION_V3)?,
        outcome_count: tail_count,
        product_record_digest: direct_identity_v3(identities, IDENTITY_PRODUCT_RECORD_DIGEST_V3)?,
        semantic_basis_id: direct_identity_v3(identities, IDENTITY_SEMANTIC_BASIS_V3)?,
        linked_basis_record_digest: direct_identity_v3(
            identities,
            IDENTITY_LINKED_BASIS_RECORD_V3,
        )?,
        trading_program: direct_identity_v3(identities, IDENTITY_TRADING_PROGRAM_V3)?,
        realm: direct_identity_v3(identities, IDENTITY_REALM_V3)?,
        mint: direct_identity_v3(identities, IDENTITY_MINT_V3)?,
        token_program: direct_identity_v3(identities, IDENTITY_TOKEN_PROGRAM_V3)?,
        buyer_maker_root: direct_identity_v3(identities, IDENTITY_BUYER_MAKER_ROOT_V3)?,
        custody_authority: direct_identity_v3(identities, IDENTITY_CUSTODY_AUTHORITY_V3)?,
        parent_request_digest: request_digest,
        claims_market_revision: direct_scalar_v3(scalars, SCALAR_CLAIMS_MARKET_REVISION_V3)?,
        seller_position_revision: direct_scalar_v3(scalars, SCALAR_SELLER_POSITION_REVISION_V3)?,
        buyer_position_revision: direct_scalar_v3(scalars, SCALAR_BUYER_POSITION_REVISION_V3)?,
        custody_revision: direct_scalar_v3(scalars, SCALAR_CUSTODY_REVISION_V3)?,
    })
}

fn direct_scalar_v3(scalars: &[u64], index: usize) -> Result<u64, ProgramError> {
    scalars
        .get(index)
        .copied()
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn direct_identity_v3(identities: &[[u8; 32]], index: usize) -> Result<[u8; 32], ProgramError> {
    identities
        .get(index)
        .copied()
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn direct_inline_participant_v3(
    request: dclutch_direct_codec::execution_v3::DirectSignedParticipantV3,
    logical_coordinate: usize,
    runtime_accounts: &[&AccountInfo<'_>],
    lifecycle_plans: &[PreparedLifecycleInvocationV3],
) -> Result<InlineParticipantV2, ProgramError> {
    let account = runtime_accounts
        .get(logical_coordinate)
        .ok_or(TradingSbfError::Content)?;
    let lifecycle = lifecycle_plans
        .iter()
        .find(|plan| plan.state == logical_coordinate)
        .ok_or(TradingSbfError::Content)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let (maker_replay, first_use) = if data.is_empty() {
        let StateLifecyclePlanV3::Create(plan) = lifecycle.plan else {
            return Err(TradingSbfError::Content.into());
        };
        (
            MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(
                plan.bump,
                account.lamports(),
            )),
            Some(MakerReplayFirstUseV1 {
                // A maker replay root is the MARKET's shared structure, and
                // this route deliberately admits a stranger as the payer of one
                // fill. `rent_owner` is a payout target for the permissionless
                // maker close, so adopting a `Payer` plan's identity here would
                // hand that stranger the rent of something the market depends
                // on. `validate_create` refuses such a plan; this is the site
                // whose consequence says why it must.
                rent_owner: plan.beneficiary,
                rent_principal: plan.historical_rent_principal,
            }),
        )
    } else {
        if !matches!(lifecycle.plan, StateLifecyclePlanV3::Authenticate(_)) {
            return Err(TradingSbfError::Content.into());
        }
        (
            MakerReplayObservationV1::Existing(
                MakerReplayRootV1::decode(&data).map_err(|_| TradingSbfError::Content)?,
            ),
            None,
        )
    };
    let authenticated =
        AuthenticatedCompactIntentV2::from_adjacent_ed25519(request.maker, request.intent)
            .map_err(|_| TradingSbfError::NativeSignature)?;
    Ok(InlineParticipantV2 {
        authenticated,
        maker_replay,
        first_use,
    })
}

fn prepare_direct_inline_collateral_v3(
    runtime_accounts: &[&AccountInfo<'_>],
    identities: &[[u8; 32]],
    context: DirectInlineCandidateContextV2,
    accounts: DirectInlinePoststateAccountsV3,
) -> Result<HeapBoxV3<DirectInlineCollateralFrameV2>, ProgramError> {
    let buyer_account = direct_runtime_account_v3(runtime_accounts, accounts.buyer_token)?;
    let seller_account = direct_runtime_account_v3(runtime_accounts, accounts.seller_token)?;
    let fee_account = direct_runtime_account_v3(runtime_accounts, accounts.fee_token)?;
    let token_program = Pubkey::new_from_array(context.token_program);
    if buyer_account.owner != &token_program
        || seller_account.owner != &token_program
        || fee_account.owner != &token_program
        || buyer_account.key.to_bytes()
            != direct_identity_v3(identities, IDENTITY_BUYER_TOKEN_ACCOUNT_V3)?
        || seller_account.key.to_bytes()
            != direct_identity_v3(identities, IDENTITY_SELLER_TOKEN_ACCOUNT_V3)?
        || fee_account.key.to_bytes()
            != direct_identity_v3(identities, IDENTITY_FEE_TOKEN_ACCOUNT_V3)?
    {
        return Err(TradingSbfError::Content.into());
    }
    let buyer = direct_token_account_v3(buyer_account)?;
    let seller = direct_token_account_v3(seller_account)?;
    let fee = direct_token_account_v3(fee_account)?;
    if buyer.mint != context.mint || seller.mint != context.mint || fee.mint != context.mint {
        return Err(TradingSbfError::Content.into());
    }
    let delegate = match buyer.delegate {
        TokenCOption::Some(delegate) => delegate,
        TokenCOption::None => return Err(TradingSbfError::Content.into()),
    };
    HeapBoxV3::new(DirectInlineCollateralFrameV2 {
        buyer_source: DirectExternalDebitV2 {
            account: buyer_account.key.to_bytes(),
            owner: buyer.owner,
            delegate,
            delegated_amount: buyer.delegated_amount,
            balance: buyer.amount,
        },
        seller_destination: DirectExternalCollateralV2 {
            account: seller_account.key.to_bytes(),
            owner: seller.owner,
            balance: seller.amount,
        },
        fee_destination: DirectExternalCollateralV2 {
            account: fee_account.key.to_bytes(),
            owner: fee.owner,
            balance: fee.amount,
        },
    })
}

fn direct_token_account_v3(account: &AccountInfo<'_>) -> Result<TokenAccount, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    TokenAccount::parse(&data).map_err(|_| TradingSbfError::Content.into())
}

fn direct_claims_local_v3(role: ClaimsFrameRoleV1) -> Result<u16, ProgramError> {
    let spec = SparseNativeTransferFrameSpecV1;
    (0..SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1)
        .find(|index| {
            spec.account(*index)
                .is_ok_and(|account| account.role() == role)
        })
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn direct_custody_local_v3(role: CustodyFrameRoleV1) -> Result<u16, ProgramError> {
    let spec = CustodyFrameSpecV1::new(OperationV1::Transfer);
    (0..TRANSFER_ACCOUNT_COUNT_V1)
        .find(|index| {
            spec.account(*index)
                .is_ok_and(|account| account.role() == role)
        })
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn direct_fixed_coordinate_v3(start: u16, local: u16) -> Result<u16, ProgramError> {
    start
        .checked_add(local)
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn direct_runtime_account_v3<'a, 'info>(
    runtime_accounts: &'a [&'a AccountInfo<'info>],
    coordinate: u16,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    runtime_accounts
        .get(usize::from(coordinate))
        .copied()
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn direct_inline_effect_dispatch_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<
    (
        DirectInlineEffectDispatchV2,
        DirectInlinePoststateAccountsV3,
    ),
    ProgramError,
> {
    if effect.route_count() != 5 {
        return Err(TradingSbfError::Content.into());
    }
    let claims = effect
        .base()
        .route(0)
        .map_err(|_| TradingSbfError::Content)?;
    if claims.role() != FixedRole::Claims
        || claims.kind() != dclutch_effect_kernel::v3::RouteKindV3::Once
        || claims.fixed_account_count() != SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1
        || effect
            .invocation_count(0, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?
            != 1
    {
        return Err(TradingSbfError::Content.into());
    }
    let mut custody_slots = [0_u8; DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2];
    let mut custody_count = 0_usize;
    let mut child_dispatch_writable = [false; DIRECT_INLINE_CUSTODY_ROUTE_SLOTS_V2];
    let mut custody_start = None;
    let mut fee_start = None;
    for slot in 0..4_usize {
        let route_index = u16::try_from(slot + 1).map_err(|_| TradingSbfError::Content)?;
        let route = effect
            .base()
            .route(route_index)
            .map_err(|_| TradingSbfError::Content)?;
        let count = effect
            .invocation_count(route_index, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?;
        if route.role() != FixedRole::Custody
            || route.kind() != dclutch_effect_kernel::v3::RouteKindV3::Once
            || route.fixed_account_count() != TRANSFER_ACCOUNT_COUNT_V1
            || count > 1
        {
            return Err(TradingSbfError::Content.into());
        }
        if slot == 0 {
            custody_start = Some(route.fixed_account_start());
        }
        if slot == 2 {
            fee_start = Some(route.fixed_account_start());
        }
        if count == 1 {
            *custody_slots
                .get_mut(custody_count)
                .ok_or(TradingSbfError::Content)? =
                u8::try_from(slot).map_err(|_| TradingSbfError::Content)?;
            *child_dispatch_writable
                .get_mut(slot)
                .ok_or(TradingSbfError::Content)? = true;
            custody_count = custody_count
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?;
        }
    }
    let custody_start = custody_start.ok_or(TradingSbfError::Content)?;
    let fee_start = fee_start.ok_or(TradingSbfError::Content)?;
    Ok((
        DirectInlineEffectDispatchV2 {
            custody_slots,
            custody_count: u8::try_from(custody_count).map_err(|_| TradingSbfError::Content)?,
            child_dispatch_writable,
        },
        DirectInlinePoststateAccountsV3 {
            claims_market: direct_fixed_coordinate_v3(
                claims.fixed_account_start(),
                direct_claims_local_v3(ClaimsFrameRoleV1::ClaimsMarket)?,
            )?,
            seller_position: direct_fixed_coordinate_v3(
                claims.fixed_account_start(),
                direct_claims_local_v3(ClaimsFrameRoleV1::SparseSourcePosition)?,
            )?,
            buyer_position: direct_fixed_coordinate_v3(
                claims.fixed_account_start(),
                direct_claims_local_v3(ClaimsFrameRoleV1::SparseDestinationPosition)?,
            )?,
            buyer_token: direct_fixed_coordinate_v3(
                custody_start,
                direct_custody_local_v3(CustodyFrameRoleV1::TransferSource)?,
            )?,
            seller_token: direct_fixed_coordinate_v3(
                custody_start,
                direct_custody_local_v3(CustodyFrameRoleV1::TransferDestination)?,
            )?,
            fee_token: direct_fixed_coordinate_v3(
                fee_start,
                direct_custody_local_v3(CustodyFrameRoleV1::TransferDestination)?,
            )?,
            custody_replay: direct_fixed_coordinate_v3(
                custody_start,
                direct_custody_local_v3(CustodyFrameRoleV1::Replay)?,
            )?,
        },
    ))
}
