//! Inline ordinary Direct planning behind the family-neutral Trading V3 hot path.
//!
//! This is a differential/executable oracle, not a family dispatch authority.
//! The selected Request/Transition/Effect programs remain the only live Trading
//! authority. They project the same exact Claims and Custody requests; the
//! common hot executor owns CPI order, receipt producer checks, and the single
//! final state commit.
//!
//! Trading-owned root/maker creation is described only by the descriptor's
//! canonical `StateLifecyclePolicyV3`. This module accepts those generic plans
//! and cross-checks them against the Direct semantic candidate; it never owns a
//! private System-program create or close path.

extern crate alloc;

use alloc::boxed::Box;
use dclutch_account_profile_contract::lifecycle_v3::{
    AuthenticateStatePlanV3, CreateStatePlanV3, StateLifecyclePlanV3,
};
use dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_claims_svm::sparse_native_transfer_v1::SparseNativeTransferV1;
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyRequestV1, OperationV1,
};
use dclutch_direct_codec::{
    inline_candidate_v2::{
        DirectInlineCandidateContextV2, encode_inline_claims_request_v2,
        prepare_inline_ordinary_candidate_v2, project_inline_custody_effect_v2,
        verify_inline_claims_receipt_v2 as verify_candidate_claims_receipt_v2,
        verify_inline_custody_receipt_v2 as verify_candidate_custody_receipt_v2,
        verify_inline_effect_partition_v2 as verify_candidate_effect_partition_v2,
    },
    successor::{
        DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_ROOT_STATE_BYTES_V1, DirectCoordinatesV1,
        InlineOrdinaryInputV2, InlineOrdinarySettlementV2, MakerReplaySeedsV1,
    },
};
use dclutch_market_core_codec::{CoreMarketViewV1, Phase};
use solana_program::pubkey::Pubkey;

use super::physical::{DirectPhysicalError, Result};

pub use dclutch_direct_codec::inline_candidate_v2::{
    DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2, DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2,
    DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3, DirectInlineCollateralFrameV2,
    DirectInlineCustodyEffectV2, DirectInlineEffectDispatchV2,
};

/// Authenticated fixed-role, replay, and revision facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlinePhysicalContextV2 {
    /// Sparse Core Market view after exact references and Registry authentication.
    pub core_market: CoreMarketViewV1,
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// Current Registry-selected Claims program.
    pub claims_program: [u8; 32],
    /// Exact Trading-owned Direct root account selected by AccountProfile V2.
    pub direct_root: [u8; 32],
    /// Canonical seller maker replay account.
    pub seller_maker_root: [u8; 32],
    /// Canonical buyer maker replay account and Custody replay context.
    pub buyer_maker_root: [u8; 32],
    /// Canonical Custody replay account for `buyer_maker_root`.
    pub custody_replay: [u8; 32],
    /// Current exact Custody replay state.
    pub custody_replay_state: CustodyReplayV1,
    /// Canonical Custody transfer authority.
    pub custody_authority: [u8; 32],
    /// SHA-256 of the complete canonical parent Trading request.
    pub parent_request_digest: [u8; 32],
    /// Exact finalized linked LiabilityBasis record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Claims aggregate revision before the transfer.
    pub claims_market_revision: u64,
    /// Seller Position revision before the transfer.
    pub seller_position_revision: u64,
    /// Buyer Position revision before the transfer.
    pub buyer_position_revision: u64,
}

/// Exact generic lifecycle plans for the root and both maker replay accounts.
///
/// The common Hot V3 outer obtains these only from `plan_lifecycle` after
/// AccountProfile projection and PDA/Rent/RentCredit authentication. Direct
/// independently checks that their economic facts equal its semantic candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineLifecyclePlansV3 {
    /// Existing composite Direct capability root authentication.
    pub root: StateLifecyclePlanV3,
    /// Existing authentication or dust-tolerant first-use creation for seller.
    pub seller_maker: StateLifecyclePlanV3,
    /// Existing authentication or dust-tolerant first-use creation for buyer.
    pub buyer_maker: StateLifecyclePlanV3,
}

/// Complete inline physical candidate. No authoritative account is mutated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlinePhysicalPlanV2 {
    /// Sole accepted Direct settlement candidate.
    pub settlement: InlineOrdinarySettlementV2,
    /// Exact generic root/maker lifecycle plans bound to the settlement.
    pub lifecycle: DirectInlineLifecyclePlansV3,
    /// Number of positive Custody transfers.
    pub custody_count: u8,
    /// Exact encoded runtime-width Claims request length.
    pub claims_bytes: usize,
    /// Buyer source token balance after all transfers.
    pub buyer_source_after: u64,
    /// Buyer residual delegated allowance after the actual debit.
    pub buyer_delegated_after: u64,
    /// Seller destination balance after all transfers.
    pub seller_destination_after: u64,
    /// Fee destination balance after all transfers.
    pub fee_destination_after: u64,
}

/// Caller-owned candidate outputs copied only after all child effects succeed.
pub struct DirectInlineStateBuffersV2<'a> {
    /// Global Direct root tail.
    pub root_output: &'a mut [u8],
    /// Seller maker replay state.
    pub seller_maker_output: &'a mut [u8],
    /// Buyer maker replay state.
    pub buyer_maker_output: &'a mut [u8],
}

/// Construct the exact existing-root inline Claims and Custody effects.
///
/// Scratch may change on refusal. `claims_output` remains unchanged on every
/// refusal. Width is derived from the authenticated Product and checked before
/// any scratch write; this function has no protocol outcome ceiling.
#[inline(never)]
pub fn prepare_inline_ordinary_physical_v2(
    direct: InlineOrdinaryInputV2,
    context: DirectInlinePhysicalContextV2,
    lifecycle: DirectInlineLifecyclePlansV3,
    collateral: DirectInlineCollateralFrameV2,
    claims_scratch: &mut [u8],
    claims_output: &mut [u8],
) -> Result<Box<DirectInlinePhysicalPlanV2>> {
    validate_context(direct, context)?;
    let candidate =
        prepare_inline_ordinary_candidate_v2(direct, candidate_context(context), collateral)?;
    validate_lifecycle(direct, context, candidate.settlement, lifecycle)?;
    validate_maker_roots(direct, context, candidate.settlement)?;
    validate_replay(context)?;

    let claims_bytes = DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2;
    if claims_scratch.len() != claims_bytes || claims_output.len() != claims_bytes {
        return Err(DirectPhysicalError::Width);
    }

    let claims_request = encode_inline_claims_request_v2(direct, candidate_context(context))?;
    claims_scratch.copy_from_slice(&claims_request);
    SparseNativeTransferV1::decode(claims_scratch).map_err(|_| DirectPhysicalError::Claims)?;
    claims_output.copy_from_slice(claims_scratch);

    Ok(Box::new(DirectInlinePhysicalPlanV2 {
        settlement: candidate.settlement,
        lifecycle,
        custody_count: candidate.custody_count,
        claims_bytes,
        buyer_source_after: candidate.buyer_source_after,
        buyer_delegated_after: candidate.buyer_delegated_after,
        seller_destination_after: candidate.seller_destination_after,
        fee_destination_after: candidate.fee_destination_after,
    }))
}

fn validate_lifecycle(
    direct: InlineOrdinaryInputV2,
    context: DirectInlinePhysicalContextV2,
    settlement: InlineOrdinarySettlementV2,
    lifecycle: DirectInlineLifecyclePlansV3,
) -> Result<()> {
    match lifecycle.root {
        StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
            state, data_bytes, ..
        }) if state == context.direct_root
            && usize::try_from(data_bytes).ok()
                == CAPABILITY_ROOT_HEADER_BYTES_V1.checked_add(DIRECT_ROOT_STATE_BYTES_V1) => {}
        StateLifecyclePlanV3::Authenticate(_)
        | StateLifecyclePlanV3::Create(_)
        | StateLifecyclePlanV3::Close(_) => return Err(DirectPhysicalError::State),
    }
    validate_maker_lifecycle(
        context.seller_maker_root,
        direct.seller.first_use,
        settlement.seller_maker_root,
        settlement.seller_creation,
        lifecycle.seller_maker,
    )?;
    validate_maker_lifecycle(
        context.buyer_maker_root,
        direct.buyer.first_use,
        settlement.buyer_maker_root,
        settlement.buyer_creation,
        lifecycle.buyer_maker,
    )
}

fn validate_maker_lifecycle(
    expected_state: [u8; 32],
    first_use: Option<dclutch_direct_codec::successor::MakerReplayFirstUseV1>,
    maker_state: dclutch_direct_codec::successor::MakerReplayRootV1,
    creation: Option<dclutch_direct_codec::successor::MakerReplayCreationPlanV1>,
    lifecycle: StateLifecyclePlanV3,
) -> Result<()> {
    match (first_use, creation, lifecycle) {
        (
            None,
            None,
            StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
                state,
                data_bytes,
                bump,
                ..
            }),
        ) if state == expected_state
            && usize::try_from(data_bytes).ok() == Some(DIRECT_MAKER_REPLAY_BYTES_V1)
            && bump == maker_state.bump() =>
        {
            Ok(())
        }
        (
            Some(first_use),
            Some(creation),
            StateLifecyclePlanV3::Create(CreateStatePlanV3 {
                state,
                payer,
                rent_credit,
                beneficiary,
                target_data_bytes,
                historical_rent_principal,
                state_before,
                state_after,
                payer_debit,
                bump,
                ..
            }),
        ) if state == expected_state
            && payer != [0; 32]
            && rent_credit != [0; 32]
            && payer != state
            && rent_credit != state
            && payer != rent_credit
            && beneficiary == first_use.rent_owner
            && usize::try_from(target_data_bytes).ok() == Some(DIRECT_MAKER_REPLAY_BYTES_V1)
            && historical_rent_principal == first_use.rent_principal
            && state_before == creation.observed_lamports
            && state_after == creation.post_lamports
            && payer_debit == creation.top_up_lamports
            && bump == maker_state.bump() =>
        {
            Ok(())
        }
        _ => Err(DirectPhysicalError::State),
    }
}

/// Verify one immediate Claims receipt against the exact inline packet.
pub fn verify_inline_claims_receipt_v2(
    context: DirectInlinePhysicalContextV2,
    claims_packet: &[u8],
    receipt_bytes: &[u8],
    expected_post_resource_digest: [u8; 32],
) -> Result<()> {
    verify_candidate_claims_receipt_v2(
        context.claims_program,
        claims_packet,
        receipt_bytes,
        expected_post_resource_digest,
    )
}

/// Verify one immediate Custody receipt and post-CPI delegate allowance.
pub fn verify_inline_custody_receipt_v2(
    effect: DirectInlineCustodyEffectV2,
    receipt_bytes: &[u8],
    replay_state_digest: [u8; 32],
    observed_delegated_after: u64,
) -> Result<()> {
    verify_candidate_custody_receipt_v2(
        effect,
        receipt_bytes,
        replay_state_digest,
        observed_delegated_after,
    )
}

/// Project one positive Custody request through the shared pure candidate.
pub fn project_inline_custody_effect_physical_v2(
    direct: InlineOrdinaryInputV2,
    context: DirectInlinePhysicalContextV2,
    collateral: DirectInlineCollateralFrameV2,
    settlement: InlineOrdinarySettlementV2,
    transfer_index: u8,
) -> Result<DirectInlineCustodyEffectV2> {
    project_inline_custody_effect_v2(
        direct,
        candidate_context(context),
        collateral,
        settlement,
        transfer_index,
    )
}

/// Cross-check the exact authenticated Effect request and dispatch partition.
pub fn verify_inline_effect_partition_physical_v2(
    direct: InlineOrdinaryInputV2,
    context: DirectInlinePhysicalContextV2,
    collateral: DirectInlineCollateralFrameV2,
    request_bank: &[u8],
    dispatch: DirectInlineEffectDispatchV2,
) -> Result<()> {
    let candidate =
        prepare_inline_ordinary_candidate_v2(direct, candidate_context(context), collateral)?;
    verify_candidate_effect_partition_v2(
        direct,
        candidate_context(context),
        collateral,
        candidate,
        request_bank,
        dispatch,
    )
}

/// Encode complete root/maker state candidates without writing accounts.
pub fn encode_inline_state_candidate_v2(
    settlement: InlineOrdinarySettlementV2,
    buffers: DirectInlineStateBuffersV2<'_>,
) -> Result<()> {
    if buffers.root_output.len() != DIRECT_ROOT_STATE_BYTES_V1
        || buffers.seller_maker_output.len() != DIRECT_MAKER_REPLAY_BYTES_V1
        || buffers.buyer_maker_output.len() != DIRECT_MAKER_REPLAY_BYTES_V1
    {
        return Err(DirectPhysicalError::Width);
    }
    let root = settlement.root.encode();
    let seller = settlement
        .seller_maker_root
        .encode()
        .map_err(|_| DirectPhysicalError::State)?;
    let buyer = settlement
        .buyer_maker_root
        .encode()
        .map_err(|_| DirectPhysicalError::State)?;
    buffers.root_output.copy_from_slice(&root);
    buffers.seller_maker_output.copy_from_slice(&seller);
    buffers.buyer_maker_output.copy_from_slice(&buyer);
    Ok(())
}

fn validate_context(
    direct: InlineOrdinaryInputV2,
    context: DirectInlinePhysicalContextV2,
) -> Result<()> {
    let release = context.core_market.release_set();
    for identity in [
        context.trading_program,
        context.claims_program,
        context.direct_root,
        context.seller_maker_root,
        context.buyer_maker_root,
        context.custody_replay,
        context.custody_authority,
        context.parent_request_digest,
        context.linked_basis_record_digest,
    ] {
        if identity == [0; 32] {
            return Err(DirectPhysicalError::ZeroIdentity);
        }
    }
    if context.core_market.phase() != Phase::Open
        || release.bindings[1].program.to_bytes() != context.claims_program
        || release.bindings[2].program.to_bytes() != context.trading_program
        || context.direct_root == context.seller_maker_root
        || context.direct_root == context.buyer_maker_root
        || context.seller_maker_root == context.buyer_maker_root
        || context.core_market.market().to_bytes() != direct.seller.authenticated.intent().market
        || context.core_market.market().to_bytes() != direct.buyer.authenticated.intent().market
        || context.core_market.generation() != direct.seller.authenticated.intent().generation
        || context.core_market.generation() != direct.buyer.authenticated.intent().generation
        || context.core_market.product().outcome_count != direct.execution.outcome_count
    {
        return Err(DirectPhysicalError::Binding);
    }
    let custody_program = release.bindings[4].program.to_bytes();
    let probe = base_custody_request(&context, 0, 1)?;
    let authority = CustodyAuthoritySeedsV1::from_request(probe);
    if derive(custody_program, &authority.as_slices()).0 != context.custody_authority {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn candidate_context(context: DirectInlinePhysicalContextV2) -> DirectInlineCandidateContextV2 {
    let release = context.core_market.release_set();
    let product = context.core_market.product();
    let realm = context.core_market.realm();
    DirectInlineCandidateContextV2 {
        release_set: release.release_set_id.to_bytes(),
        market: context.core_market.market().to_bytes(),
        generation: context.core_market.generation(),
        outcome_count: product.outcome_count,
        product_record_digest: product.product_record.to_bytes(),
        semantic_basis_id: product.liability_basis.to_bytes(),
        linked_basis_record_digest: context.linked_basis_record_digest,
        trading_program: context.trading_program,
        realm: realm.realm_id.to_bytes(),
        mint: realm.collateral_mint.to_bytes(),
        token_program: realm.token_program.to_bytes(),
        buyer_maker_root: context.buyer_maker_root,
        custody_authority: context.custody_authority,
        parent_request_digest: context.parent_request_digest,
        claims_market_revision: context.claims_market_revision,
        seller_position_revision: context.seller_position_revision,
        buyer_position_revision: context.buyer_position_revision,
        custody_revision: context.custody_replay_state.next_revision,
    }
}

/// Reproduce both maker replay PDAs at the bumps the settlement carries.
///
/// This is the [`hot_v3`](crate::hot_v3) `borrow_finalized_record_at` argument
/// again: the outer preplan is the walk that SEARCHES (its `plan_lifecycle`
/// derives each maker replay canonically after AccountProfile projection), and
/// `validate_lifecycle` has already required each settlement bump to equal
/// that plan's authenticated bump before this runs. So the two
/// `find_program_address` searches this function used to repeat are replaced
/// by the two `create_program_address` calls they would have ended on. Nothing
/// in the conjunction weakens: the derivation from the exact authenticated
/// seeds plus the carried bump must still reproduce the exact context account,
/// and a wrong, noncanonical, or substituted-coordinate bump reproduces a
/// different address (or none at all) and refuses — the derivation IS the
/// check, and the carried bump is a memo of the preplan's own search, never an
/// authority. Only who pays for the search changed: per-maker searches were a
/// per-draw CU variance on the 1.4M ceiling.
fn validate_maker_roots(
    direct: InlineOrdinaryInputV2,
    context: DirectInlinePhysicalContextV2,
    settlement: InlineOrdinarySettlementV2,
) -> Result<()> {
    let coordinates = DirectCoordinatesV1::new(
        context.core_market.market().to_bytes(),
        context.core_market.generation(),
    )
    .map_err(|_| DirectPhysicalError::State)?;
    let trading_program = Pubkey::new_from_array(context.trading_program);
    for (maker, key, state) in [
        (
            direct.seller.authenticated.maker(),
            context.seller_maker_root,
            settlement.seller_maker_root,
        ),
        (
            direct.buyer.authenticated.maker(),
            context.buyer_maker_root,
            settlement.buyer_maker_root,
        ),
    ] {
        let seeds =
            MakerReplaySeedsV1::new(coordinates, maker).map_err(|_| DirectPhysicalError::State)?;
        let [domain, market, generation, maker_seed] = seeds.as_slices();
        let bump_seed = [state.bump()];
        let reproduced = Pubkey::create_program_address(
            &[domain, market, generation, maker_seed, &bump_seed],
            &trading_program,
        )
        .map_err(|_| DirectPhysicalError::Binding)?;
        if key != reproduced.to_bytes() {
            return Err(DirectPhysicalError::Binding);
        }
    }
    Ok(())
}

fn validate_replay(context: DirectInlinePhysicalContextV2) -> Result<()> {
    let replay = context.custody_replay_state;
    replay
        .to_bytes()
        .map_err(|_| DirectPhysicalError::Binding)?;
    let release = context.core_market.release_set().release_set_id.to_bytes();
    let realm = context.core_market.realm().realm_id.to_bytes();
    let custody_program = context.core_market.release_set().bindings[4]
        .program
        .to_bytes();
    if replay.caller_role != CallerRoleV1::Trading
        || replay.release_set != release
        || replay.market != context.core_market.market().to_bytes()
        || replay.realm != realm
        || replay.context != context.buyer_maker_root
        || replay.caller_program != context.trading_program
        || replay.open_vault_count != 0
        || replay.generation != context.core_market.generation()
    {
        return Err(DirectPhysicalError::Binding);
    }
    let probe = base_custody_request(&context, 0, 0)?;
    let replay_seeds = CustodyReplaySeedsV1::from_request(probe);
    if derive(custody_program, &replay_seeds.as_slices()).0 != context.custody_replay {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn base_custody_request(
    context: &DirectInlinePhysicalContextV2,
    transfer_index: usize,
    amount: u64,
) -> Result<CustodyRequestV1> {
    let realm = context.core_market.realm();
    Ok(CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::External,
        destination_compartment: CompartmentV1::External,
        release_set: context.core_market.release_set().release_set_id.to_bytes(),
        market: context.core_market.market().to_bytes(),
        realm: realm.realm_id.to_bytes(),
        context: context.buyer_maker_root,
        caller_program: context.trading_program,
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [1; 32],
            destination_owner: [2; 32],
            // Inline ordinary has no separately persisted order record. The
            // complete authenticated Direct request is therefore the sole
            // canonical order coordinate as well as the parent request.
            order: context.parent_request_digest,
            parent_request_digest: context.parent_request_digest,
            order_nonce: 0,
            generation: context.core_market.generation(),
            page_index: 0,
            execution_index: 0,
            transfer_index: u16::try_from(transfer_index)
                .map_err(|_| DirectPhysicalError::Arithmetic)?,
        },
        source: [1; 32],
        destination: [2; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: realm.collateral_mint.to_bytes(),
        token_program: realm.token_program.to_bytes(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: context.custody_replay_state.next_revision,
        resulting_revision: checked_next(context.custody_replay_state.next_revision)?,
        amount,
        rent_lamports: 0,
    })
}

fn checked_next(value: u64) -> Result<u64> {
    value.checked_add(1).ok_or(DirectPhysicalError::Arithmetic)
}

fn derive(program: [u8; 32], seeds: &[&[u8]]) -> ([u8; 32], u8) {
    let program = Pubkey::new_from_array(program);
    let (key, bump) = Pubkey::find_program_address(seeds, &program);
    (key.to_bytes(), bump)
}
