//! Inline ordinary Direct planning behind the family-neutral Trading V3 hot path.
//!
//! This is a differential/executable oracle, not a family dispatch authority.
//! The selected Request/Transition/Effect programs remain the only live Trading
//! authority. They project the same exact Claims and Custody requests; the
//! common hot executor owns CPI order, receipt producer checks, and the single
//! final state commit.
//!
//! This profile requires an existing Direct root, both existing maker replay
//! roots, and an existing zero-Vault Custody replay keyed by the buyer maker
//! root. The common effect language cannot yet allocate/assign or close
//! Trading-owned accounts, so first-use remains fail-closed rather than being
//! implemented by a Direct-private System-program path.

use dclutch_claims_svm::{
    CLAIMS_PLAN_HEADER_BYTES_V1, CallerRole as ClaimsCallerRole, ClaimsAction, ClaimsPlanV1,
    ClaimsReceiptV1,
};
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1, CustodyReceiptV1,
    CustodyReplaySeedsV1, CustodyReplayV1, CustodyRequestV1, OperationV1,
};
use dclutch_direct_codec::successor::{
    DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_ROOT_STATE_BYTES_V1, DirectCoordinatesV1,
    InlineOrdinaryInputV2, InlineOrdinarySettlementV2, MakerReplaySeedsV1,
    settle_inline_ordinary_v2,
};
use dclutch_market_core_codec::{CoreMarketViewV1, Phase};
use solana_program::{hash::hash, pubkey::Pubkey};

use super::physical::{
    DirectExternalCollateralV2, DirectExternalDebitV2, DirectPhysicalError, Result,
};

/// Seller-net and combined-fee are the only positive inline collateral routes.
pub const DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2: usize = 2;

/// Exact external token observations for one inline ordinary match.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineCollateralFrameV2 {
    /// Buyer-signed external source and current Custody delegation.
    pub buyer_source: DirectExternalDebitV2,
    /// Seller-signed external destination.
    pub seller_destination: DirectExternalCollateralV2,
    /// Immutable config-recipient external destination.
    pub fee_destination: DirectExternalCollateralV2,
}

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
    /// Claims aggregate revision before the transfer.
    pub claims_market_revision: u64,
    /// Seller Position revision before the transfer.
    pub seller_position_revision: u64,
    /// Buyer Position revision before the transfer.
    pub buyer_position_revision: u64,
}

/// One exact Custody request and its required token/delegate poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineCustodyEffectV2 {
    /// Canonical distinct-owner Custody transfer request.
    pub request: CustodyRequestV1,
    /// Exact source token balance after this CPI.
    pub source_after: u64,
    /// Exact source delegated allowance after this CPI.
    pub delegated_after: u64,
    /// Exact destination token balance after this CPI.
    pub destination_after: u64,
}

/// Complete inline physical candidate. No authoritative account is mutated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlinePhysicalPlanV2 {
    /// Sole accepted Direct settlement candidate.
    pub settlement: InlineOrdinarySettlementV2,
    /// Positive seller-net then combined-fee Custody transfers.
    pub custody: [Option<DirectInlineCustodyEffectV2>; DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2],
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
pub fn prepare_inline_ordinary_physical_v2(
    direct: InlineOrdinaryInputV2,
    context: DirectInlinePhysicalContextV2,
    collateral: DirectInlineCollateralFrameV2,
    quantity_scratch: &mut [u8],
    claims_scratch: &mut [u8],
    claims_output: &mut [u8],
) -> Result<DirectInlinePhysicalPlanV2> {
    validate_context(direct, context)?;
    let settlement =
        settle_inline_ordinary_v2(direct).map_err(|_| DirectPhysicalError::Settlement)?;
    if settlement.seller_creation.is_some() || settlement.buyer_creation.is_some() {
        return Err(DirectPhysicalError::LifecycleUnavailable);
    }
    validate_maker_roots(direct, context, settlement)?;
    validate_replay(context)?;
    validate_collateral(direct, context, collateral, settlement)?;

    let tail_bytes = usize::try_from(direct.execution.outcome_count)
        .map_err(|_| DirectPhysicalError::Width)?
        .checked_mul(8)
        .ok_or(DirectPhysicalError::Width)?;
    let claims_bytes = CLAIMS_PLAN_HEADER_BYTES_V1
        .checked_add(tail_bytes)
        .ok_or(DirectPhysicalError::Width)?;
    if quantity_scratch.len() != tail_bytes
        || claims_scratch.len() != claims_bytes
        || claims_output.len() != claims_bytes
    {
        return Err(DirectPhysicalError::Width);
    }

    let custody = compile_custody(direct, context, collateral, settlement)?;
    quantity_scratch.fill(0);
    let quantity_offset = usize::try_from(direct.seller.authenticated.intent().outcome)
        .map_err(|_| DirectPhysicalError::Width)?
        .checked_mul(8)
        .ok_or(DirectPhysicalError::Width)?;
    quantity_scratch
        .get_mut(
            quantity_offset
                ..quantity_offset
                    .checked_add(8)
                    .ok_or(DirectPhysicalError::Width)?,
        )
        .ok_or(DirectPhysicalError::Width)?
        .copy_from_slice(&direct.execution.fill.to_le_bytes());
    let claims = ClaimsPlanV1::new(
        ClaimsAction::TransferNative,
        ClaimsCallerRole::Trading,
        context.core_market.release_set().release_set_id.to_bytes(),
        context.core_market.market().to_bytes(),
        context.parent_request_digest,
        direct.seller.authenticated.maker(),
        direct.buyer.authenticated.maker(),
        context.claims_market_revision,
        context.seller_position_revision,
        context.buyer_position_revision,
        direct.execution.outcome_count,
        quantity_scratch,
    )
    .map_err(|_| DirectPhysicalError::Claims)?;
    claims
        .encode_into(claims_scratch)
        .map_err(|_| DirectPhysicalError::Claims)?;
    ClaimsPlanV1::decode(claims_scratch).map_err(|_| DirectPhysicalError::Claims)?;
    claims_output.copy_from_slice(claims_scratch);

    Ok(DirectInlinePhysicalPlanV2 {
        settlement,
        custody: custody.effects,
        custody_count: custody.count,
        claims_bytes,
        buyer_source_after: custody.source_after,
        buyer_delegated_after: custody.delegated_after,
        seller_destination_after: custody.seller_after,
        fee_destination_after: custody.fee_after,
    })
}

/// Verify one immediate Claims receipt against the exact inline packet.
pub fn verify_inline_claims_receipt_v2(
    context: DirectInlinePhysicalContextV2,
    claims_packet: &[u8],
    receipt_bytes: &[u8],
) -> Result<()> {
    let plan = ClaimsPlanV1::decode(claims_packet).map_err(|_| DirectPhysicalError::Claims)?;
    let receipt =
        ClaimsReceiptV1::decode(receipt_bytes).map_err(|_| DirectPhysicalError::Claims)?;
    if receipt.caller_role() != ClaimsCallerRole::Trading
        || receipt.action() != ClaimsAction::TransferNative
        || receipt.release_set_id() != context.core_market.release_set().release_set_id.to_bytes()
        || receipt.market() != context.core_market.market().to_bytes()
        || receipt.request_id() != context.parent_request_digest
        || receipt.packet_digest() != hash(claims_packet).to_bytes()
        || receipt.claims_program() != context.claims_program
        || receipt.pre_market_revision() != context.claims_market_revision
        || receipt.post_market_revision() != checked_next(context.claims_market_revision)?
        || receipt.post_source_revision() != checked_next(context.seller_position_revision)?
        || receipt.post_destination_revision() != checked_next(context.buyer_position_revision)?
        || receipt.payout() != 0
        || plan.source_owner() == plan.destination_owner()
    {
        return Err(DirectPhysicalError::Postcondition);
    }
    Ok(())
}

/// Verify one immediate Custody receipt and post-CPI delegate allowance.
pub fn verify_inline_custody_receipt_v2(
    effect: DirectInlineCustodyEffectV2,
    receipt_bytes: &[u8],
    replay_state_digest: [u8; 32],
    observed_delegated_after: u64,
) -> Result<()> {
    let request_bytes = effect
        .request
        .to_bytes()
        .map_err(|_| DirectPhysicalError::Custody)?;
    let receipt =
        CustodyReceiptV1::decode(receipt_bytes).map_err(|_| DirectPhysicalError::Custody)?;
    receipt
        .verify_for(
            effect.request,
            hash(&request_bytes).to_bytes(),
            replay_state_digest,
        )
        .map_err(|_| DirectPhysicalError::Custody)?;
    if receipt.evidence.source_after != effect.source_after
        || receipt.evidence.destination_after != effect.destination_after
        || observed_delegated_after != effect.delegated_after
    {
        return Err(DirectPhysicalError::Postcondition);
    }
    Ok(())
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
    let probe = base_custody_request(context, 0, 1, [1; 32])?;
    let authority = CustodyAuthoritySeedsV1::from_request(probe);
    if derive(custody_program, &authority.as_slices()).0 != context.custody_authority {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

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
        let (expected, bump) = derive(context.trading_program, &seeds.as_slices());
        if key != expected || state.bump() != bump {
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
    let probe = base_custody_request(context, 0, 0, [1; 32])?;
    let replay_seeds = CustodyReplaySeedsV1::from_request(probe);
    if derive(custody_program, &replay_seeds.as_slices()).0 != context.custody_replay {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn validate_collateral(
    direct: InlineOrdinaryInputV2,
    context: DirectInlinePhysicalContextV2,
    collateral: DirectInlineCollateralFrameV2,
    settlement: InlineOrdinarySettlementV2,
) -> Result<()> {
    let seller = direct.seller.authenticated;
    let buyer = direct.buyer.authenticated;
    for identity in [
        collateral.buyer_source.account,
        collateral.buyer_source.owner,
        collateral.buyer_source.delegate,
        collateral.seller_destination.account,
        collateral.seller_destination.owner,
        collateral.fee_destination.account,
        collateral.fee_destination.owner,
    ] {
        if identity == [0; 32] {
            return Err(DirectPhysicalError::ZeroIdentity);
        }
    }
    if collateral.buyer_source.account != buyer.intent().collateral_account
        || collateral.buyer_source.owner != buyer.maker()
        || collateral.buyer_source.delegate != context.custody_authority
        || collateral.buyer_source.delegated_amount < settlement.effects.buyer_collateral_debit
        || collateral.buyer_source.balance < settlement.effects.buyer_collateral_debit
        || collateral.seller_destination.account != seller.intent().collateral_account
        || collateral.seller_destination.owner != seller.maker()
        || collateral.fee_destination.owner != direct.execution.config.fee_recipient()
        || collateral.buyer_source.account == collateral.seller_destination.account
        || collateral.buyer_source.account == collateral.fee_destination.account
        || (collateral.seller_destination.account == collateral.fee_destination.account
            && collateral.seller_destination != collateral.fee_destination)
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn compile_custody(
    direct: InlineOrdinaryInputV2,
    context: DirectInlinePhysicalContextV2,
    collateral: DirectInlineCollateralFrameV2,
    settlement: InlineOrdinarySettlementV2,
) -> Result<CustodyCompilationV2> {
    let mut effects = [None; DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2];
    let mut count = 0_usize;
    let mut source_after = collateral.buyer_source.balance;
    let mut delegated_after = collateral.buyer_source.delegated_amount;
    let mut seller_after = collateral.seller_destination.balance;
    let mut fee_after =
        if collateral.seller_destination.account == collateral.fee_destination.account {
            seller_after
        } else {
            collateral.fee_destination.balance
        };
    for (amount, destination) in [
        (
            settlement.effects.seller_net_collateral_credit,
            collateral.seller_destination,
        ),
        (
            settlement.effects.total_fee_transfer,
            collateral.fee_destination,
        ),
    ] {
        if amount == 0 {
            continue;
        }
        let destination_before = if destination.account == collateral.seller_destination.account {
            seller_after
        } else {
            fee_after
        };
        source_after = source_after
            .checked_sub(amount)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        delegated_after = delegated_after
            .checked_sub(amount)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        let destination_after = destination_before
            .checked_add(amount)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        let request = custody_request(
            direct,
            context,
            collateral.buyer_source,
            destination,
            count,
            amount,
        )?;
        *effects
            .get_mut(count)
            .ok_or(DirectPhysicalError::Arithmetic)? = Some(DirectInlineCustodyEffectV2 {
            request,
            source_after,
            delegated_after,
            destination_after,
        });
        count = count
            .checked_add(1)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        if destination.account == collateral.seller_destination.account {
            seller_after = destination_after;
        }
        if destination.account == collateral.fee_destination.account {
            fee_after = destination_after;
        }
    }
    if source_after
        != collateral
            .buyer_source
            .balance
            .checked_sub(settlement.effects.buyer_collateral_debit)
            .ok_or(DirectPhysicalError::Arithmetic)?
        || delegated_after
            != collateral
                .buyer_source
                .delegated_amount
                .checked_sub(settlement.effects.buyer_collateral_debit)
                .ok_or(DirectPhysicalError::Arithmetic)?
    {
        return Err(DirectPhysicalError::Postcondition);
    }
    Ok(CustodyCompilationV2 {
        effects,
        count: u8::try_from(count).map_err(|_| DirectPhysicalError::Arithmetic)?,
        source_after,
        delegated_after,
        seller_after,
        fee_after,
    })
}

struct CustodyCompilationV2 {
    effects: [Option<DirectInlineCustodyEffectV2>; DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2],
    count: u8,
    source_after: u64,
    delegated_after: u64,
    seller_after: u64,
    fee_after: u64,
}

fn custody_request(
    direct: InlineOrdinaryInputV2,
    context: DirectInlinePhysicalContextV2,
    source: DirectExternalDebitV2,
    destination: DirectExternalCollateralV2,
    transfer_index: usize,
    amount: u64,
) -> Result<CustodyRequestV1> {
    let buyer_intent = direct.buyer.authenticated.intent();
    let order = hash(
        &buyer_intent
            .signed_preimage()
            .map_err(|_| DirectPhysicalError::Binding)?,
    )
    .to_bytes();
    let expected_revision = context
        .custody_replay_state
        .next_revision
        .checked_add(u64::try_from(transfer_index).map_err(|_| DirectPhysicalError::Arithmetic)?)
        .ok_or(DirectPhysicalError::Arithmetic)?;
    let mut request = base_custody_request(context, transfer_index, amount, order)?;
    request.semantic.source_owner = source.owner;
    request.semantic.destination_owner = destination.owner;
    request.semantic.order_nonce = buyer_intent.nonce;
    request.semantic.execution_index = buyer_intent.outcome;
    request.source = source.account;
    request.destination = destination.account;
    request.expected_revision = expected_revision;
    request.resulting_revision = checked_next(expected_revision)?;
    request
        .validate()
        .map_err(|_| DirectPhysicalError::Custody)?;
    Ok(request)
}

fn base_custody_request(
    context: DirectInlinePhysicalContextV2,
    transfer_index: usize,
    amount: u64,
    order: [u8; 32],
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
            order,
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
