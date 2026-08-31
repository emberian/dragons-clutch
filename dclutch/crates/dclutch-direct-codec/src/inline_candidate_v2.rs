//! Shared pure candidate for one ordinary inline Direct execution.
//!
//! The selected Transition and Effect programs remain the live execution
//! authority. This module is an independent typed cross-check used by both the
//! Trading adapter and exterior operators: it settles the signed pair, emits
//! the exact Claims/Custody child requests, and predicts every Direct-owned and
//! token/revision poststate without reading accounts, deriving PDAs, invoking
//! CPI, or mutating caller buffers.

use dclutch_claims_svm::{
    CallerRole as ClaimsCallerRole,
    sparse_native_transfer_v1::{
        SPARSE_NATIVE_TRANSFER_BYTES_V1, SparseNativeTransferInputV1,
        SparseNativeTransferReceiptV1, SparseNativeTransferV1,
    },
};
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyRequestV1, DELEGATED_CUSTODY_REQUEST_BYTES_V2,
    DelegatedCustodyReceiptV2, DelegatedCustodyRequestV2, OperationV1,
};
use dclutch_sha256_adapter::digest;

use crate::successor::{
    InlineOrdinaryInputV2, InlineOrdinarySettlementV2, settle_inline_ordinary_v2,
};

/// The seller-net leg is the only Custody route this transaction dispatches.
///
/// The fee leg settles in a second transaction
/// (`docs/design/FEE_SECOND_TRANSACTION_V1.md`): the fee-bearing route did not
/// fit the compute ceiling with both legs in one transaction, by a margin
/// larger than the fee leg itself
/// (`docs/evidence/DIRECT_HOT_FEE_BEARING_CU_2026_08_30.md`). So tx1 moves
/// `seller_net`, leaves the residual allowance in place by running the seller
/// leg NON-terminally, and records the obligation on the buyer's maker replay.
pub const DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2: usize = 1;
/// Exact fixed width of the ordinary sparse Claims request.
pub const DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2: usize = SPARSE_NATIVE_TRANSFER_BYTES_V1;
/// Exact authenticated Effect request-bank width for ordinary Direct.
pub const DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3: usize =
    SPARSE_NATIVE_TRANSFER_BYTES_V1 + 4 * DELEGATED_CUSTODY_REQUEST_BYTES_V2;
/// Exact count of declared Custody route slots.
///
/// Four are declared and two are reachable. Slot 2, fee continuation, is the
/// leg that moved to the second transaction; slot 3, `FeeSole`, is retired by
/// the `DIRECT_MAX_FEE_BASIS_POINTS_V1` band and refuses at
/// [`DirectInlineCandidateErrorV2::FeeSoleRetired`]. Both keep their declared
/// slot and their physical Effect frame in this lane: renumbering the Effect's
/// 91-account topology is a separate, wider change than the protocol tier.
pub const DIRECT_INLINE_CUSTODY_ROUTE_SLOTS_V2: usize = 4;

/// Stable refusal from the pure ordinary Direct candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineCandidateErrorV2 {
    /// A required semantic, program, account, or digest identity was zero.
    ZeroIdentity,
    /// Market, generation, maker, config, or endpoint bindings differed.
    Binding,
    /// Runtime geometry or caller-owned output buffers had another exact width.
    Width,
    /// Checked balance, revision, or route arithmetic failed.
    Arithmetic,
    /// The sole Direct successor settlement refused.
    Settlement,
    /// Canonical Claims construction or receipt verification refused.
    Claims,
    /// Canonical Custody construction or receipt verification refused.
    Custody,
    /// A child receipt or observed poststate differed from the exact plan.
    Postcondition,
    /// Exact Direct state-candidate encoding refused.
    State,
    /// A settlement asked for the retired fee-only Custody route.
    ///
    /// `CUSTODY_ROUTES_V3` slot 3 needs `seller_net == 0 && combined_fee != 0`,
    /// which for a positive gross forces a rate of exactly 10,000 basis points
    /// -- five times the `DIRECT_MAX_FEE_BASIS_POINTS_V1` band decision 0014 D2
    /// adopted. Unreachable is not the same as unrepresentable, so the state
    /// gets a refusal rather than a route that silently plans nothing.
    FeeSoleRetired,
}

/// Result alias for the pure ordinary Direct candidate.
pub type Result<T> = core::result::Result<T, DirectInlineCandidateErrorV2>;

/// One authenticated external collateral token account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectExternalCollateralV2 {
    /// Exact token-account key.
    pub account: [u8; 32],
    /// Exact persisted token authority.
    pub owner: [u8; 32],
    /// Authenticated token amount before execution.
    pub balance: u64,
}

/// One authenticated external collateral source delegated to Custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectExternalDebitV2 {
    /// Exact token-account key.
    pub account: [u8; 32],
    /// Exact persisted token authority.
    pub owner: [u8; 32],
    /// Canonical Custody transfer-authority PDA.
    pub delegate: [u8; 32],
    /// Exact remaining delegated allowance before execution.
    pub delegated_amount: u64,
    /// Authenticated token amount before execution.
    pub balance: u64,
}

/// Exact external-token observations for one ordinary inline match.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineCollateralFrameV2 {
    /// Buyer-signed external source and current Custody delegation.
    pub buyer_source: DirectExternalDebitV2,
    /// Seller-signed external destination.
    pub seller_destination: DirectExternalCollateralV2,
    /// Exact selected fee destination owned by the configured recipient.
    pub fee_destination: DirectExternalCollateralV2,
}

/// PDA-free, account-owner-authenticated semantic context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineCandidateContextV2 {
    /// Current execution release-set identity.
    pub release_set: [u8; 32],
    /// Current Core Market identity.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Product-authenticated outcome count.
    pub outcome_count: u32,
    /// Finalized Product record digest.
    pub product_record_digest: [u8; 32],
    /// Product-authenticated semantic LiabilityBasis identity.
    pub semantic_basis_id: [u8; 32],
    /// Finalized linked LiabilityBasis record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// Immutable Realm identity.
    pub realm: [u8; 32],
    /// Realm-selected collateral mint.
    pub mint: [u8; 32],
    /// Realm-selected token program.
    pub token_program: [u8; 32],
    /// Buyer maker replay root and Custody replay context.
    pub buyer_maker_root: [u8; 32],
    /// Canonical Custody transfer authority.
    pub custody_authority: [u8; 32],
    /// SHA-256 of the complete canonical parent Direct request.
    pub parent_request_digest: [u8; 32],
    /// Claims aggregate revision before the transfer.
    pub claims_market_revision: u64,
    /// Seller Position revision before the transfer.
    pub seller_position_revision: u64,
    /// Buyer Position revision before the transfer.
    pub buyer_position_revision: u64,
    /// Trading-role Custody replay revision before the first positive transfer.
    pub custody_revision: u64,
}

/// One exact Custody request and its required token/delegate poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineCustodyEffectV2 {
    /// Canonical delegated-allowance Custody V2 transfer request.
    pub request: DelegatedCustodyRequestV2,
    /// Exact source token balance after this CPI.
    pub source_after: u64,
    /// Exact source delegated allowance after this CPI.
    pub delegated_after: u64,
    /// Exact destination token balance after this CPI.
    pub destination_after: u64,
}

/// Complete pure ordinary Direct candidate and exact poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineCandidateV2 {
    /// Sole accepted Direct settlement candidate.
    pub settlement: InlineOrdinarySettlementV2,
    /// Number of positive Custody transfers.
    pub custody_count: u8,
    /// Claims aggregate revision after the transfer.
    pub claims_market_revision_after: u64,
    /// Seller Position revision after the transfer.
    pub seller_position_revision_after: u64,
    /// Buyer Position revision after the transfer.
    pub buyer_position_revision_after: u64,
    /// Custody replay revision after every positive collateral transfer.
    pub custody_revision_after: u64,
    /// Buyer source token amount after every transfer.
    pub buyer_source_after: u64,
    /// Buyer residual delegated allowance after every transfer.
    pub buyer_delegated_after: u64,
    /// Seller destination token amount after every transfer.
    pub seller_destination_after: u64,
    /// Fee destination token amount after every transfer.
    pub fee_destination_after: u64,
}

/// Actual child-dispatch partition resolved from the authenticated Effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineEffectDispatchV2 {
    /// Enabled Custody route slots in exact Effect execution order.
    pub custody_slots: [u8; DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2],
    /// Exact prefix length of `custody_slots`.
    pub custody_count: u8,
    /// Whether each Custody route frame can participate in a child CPI.
    /// Inactive frames must remain false even if their raw request-bank bytes
    /// happen to decode or their physical accounts alias another coordinate.
    pub child_dispatch_writable: [bool; DIRECT_INLINE_CUSTODY_ROUTE_SLOTS_V2],
}

/// Construct one exact ordinary candidate without account or PDA authority.
#[inline(never)]
pub fn prepare_inline_ordinary_candidate_v2(
    direct: InlineOrdinaryInputV2,
    context: DirectInlineCandidateContextV2,
    collateral: DirectInlineCollateralFrameV2,
) -> Result<DirectInlineCandidateV2> {
    validate_context(direct, context)?;
    let settlement =
        settle_inline_ordinary_v2(direct).map_err(|_| DirectInlineCandidateErrorV2::Settlement)?;
    validate_collateral(direct, context, collateral, settlement)?;
    let custody = compile_custody(&direct, &context, &collateral, &settlement)?;
    let claims_market_revision_after = checked_next(context.claims_market_revision)?;
    let seller_position_revision_after = checked_next(context.seller_position_revision)?;
    let buyer_position_revision_after = checked_next(context.buyer_position_revision)?;
    let custody_revision_after = context
        .custody_revision
        .checked_add(u64::from(custody.count))
        .ok_or(DirectInlineCandidateErrorV2::Arithmetic)?;
    Ok(DirectInlineCandidateV2 {
        settlement,
        custody_count: custody.count,
        claims_market_revision_after,
        seller_position_revision_after,
        buyer_position_revision_after,
        custody_revision_after,
        buyer_source_after: custody.source_after,
        buyer_delegated_after: custody.delegated_after,
        seller_destination_after: custody.seller_after,
        fee_destination_after: custody.fee_after,
    })
}

/// Verify one immediate Claims receipt against the exact candidate packet.
pub fn verify_inline_claims_receipt_v2(
    claims_program: [u8; 32],
    claims_packet: &[u8],
    receipt_bytes: &[u8],
    expected_post_resource_digest: [u8; 32],
) -> Result<()> {
    if claims_program == [0; 32] || expected_post_resource_digest == [0; 32] {
        return Err(DirectInlineCandidateErrorV2::ZeroIdentity);
    }
    let plan = SparseNativeTransferV1::decode(claims_packet)
        .map_err(|_| DirectInlineCandidateErrorV2::Claims)?;
    let receipt = SparseNativeTransferReceiptV1::decode(receipt_bytes)
        .map_err(|_| DirectInlineCandidateErrorV2::Claims)?;
    receipt
        .validate_request(plan)
        .map_err(|_| DirectInlineCandidateErrorV2::Claims)?;
    if receipt.packet_digest() != digest(claims_packet)
        || receipt.claims_program() != claims_program
        || receipt.post_resource_digest() != expected_post_resource_digest
    {
        return Err(DirectInlineCandidateErrorV2::Postcondition);
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
        .encode()
        .map_err(|_| DirectInlineCandidateErrorV2::Custody)?;
    let receipt = DelegatedCustodyReceiptV2::decode(receipt_bytes)
        .map_err(|_| DirectInlineCandidateErrorV2::Custody)?;
    receipt
        .custody
        .verify_for(
            effect.request.custody,
            digest(&request_bytes),
            replay_state_digest,
        )
        .map_err(|_| DirectInlineCandidateErrorV2::Custody)?;
    if receipt.starts_atomic_debit != effect.request.starts_atomic_debit
        || receipt.terminal != effect.request.terminal
        || receipt.delegate_before != effect.request.delegate_before
        || receipt.delegate_after != effect.request.delegate_after
        || receipt.total_debit != effect.request.total_debit
        || receipt.allowance_before != effect.request.allowance_before
        || receipt.allowance_after != effect.request.allowance_after
        || receipt.custody.evidence.source_after != effect.source_after
        || receipt.custody.evidence.destination_after != effect.destination_after
        || receipt.allowance_after != effect.delegated_after
        || observed_delegated_after != effect.delegated_after
    {
        return Err(DirectInlineCandidateErrorV2::Postcondition);
    }
    Ok(())
}

#[inline(never)]
fn validate_context(
    direct: InlineOrdinaryInputV2,
    context: DirectInlineCandidateContextV2,
) -> Result<()> {
    for identity in [
        context.release_set,
        context.market,
        context.product_record_digest,
        context.semantic_basis_id,
        context.linked_basis_record_digest,
        context.trading_program,
        context.realm,
        context.mint,
        context.token_program,
        context.buyer_maker_root,
        context.custody_authority,
        context.parent_request_digest,
    ] {
        if identity == [0; 32] {
            return Err(DirectInlineCandidateErrorV2::ZeroIdentity);
        }
    }
    if context.market != direct.seller.authenticated.intent().market
        || context.market != direct.buyer.authenticated.intent().market
        || context.generation != direct.seller.authenticated.intent().generation
        || context.generation != direct.buyer.authenticated.intent().generation
        || context.outcome_count != direct.execution.outcome_count
    {
        return Err(DirectInlineCandidateErrorV2::Binding);
    }
    Ok(())
}

#[inline(never)]
fn validate_collateral(
    direct: InlineOrdinaryInputV2,
    context: DirectInlineCandidateContextV2,
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
            return Err(DirectInlineCandidateErrorV2::ZeroIdentity);
        }
    }
    if collateral.buyer_source.account != buyer.intent().collateral_account
        || collateral.buyer_source.owner != buyer.maker()
        || collateral.buyer_source.delegate != context.custody_authority
        || collateral.buyer_source.delegated_amount != settlement.effects.buyer_collateral_debit
        || collateral.buyer_source.balance < settlement.effects.buyer_collateral_debit
        || collateral.seller_destination.account != seller.intent().collateral_account
        || collateral.seller_destination.owner != seller.maker()
        || collateral.fee_destination.owner != direct.execution.config.fee_recipient()
        || collateral.buyer_source.account == collateral.seller_destination.account
        || collateral.buyer_source.account == collateral.fee_destination.account
        || (collateral.seller_destination.account == collateral.fee_destination.account
            && collateral.seller_destination != collateral.fee_destination)
    {
        return Err(DirectInlineCandidateErrorV2::Binding);
    }
    Ok(())
}

/// Encode the exact sparse Claims request for a prepared Direct candidate.
#[inline(never)]
pub fn encode_inline_claims_request_v2(
    direct: InlineOrdinaryInputV2,
    context: DirectInlineCandidateContextV2,
) -> Result<[u8; DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2]> {
    SparseNativeTransferV1::new(SparseNativeTransferInputV1 {
        caller_role: ClaimsCallerRole::Trading,
        release_set: context.release_set,
        market: context.market,
        request_id: context.parent_request_digest,
        product_record_digest: context.product_record_digest,
        semantic_basis_id: context.semantic_basis_id,
        linked_basis_record_digest: context.linked_basis_record_digest,
        source_owner: direct.seller.authenticated.maker(),
        destination_owner: direct.buyer.authenticated.maker(),
        expected_market_revision: context.claims_market_revision,
        expected_source_revision: context.seller_position_revision,
        expected_destination_revision: context.buyer_position_revision,
        generation: context.generation,
        outcome: direct.seller.authenticated.intent().outcome,
        claim_count: context.outcome_count,
        quantity: direct.execution.fill,
    })
    .map(SparseNativeTransferV1::to_bytes)
    .map_err(|_| DirectInlineCandidateErrorV2::Claims)
}

#[inline(never)]
fn compile_custody(
    direct: &InlineOrdinaryInputV2,
    context: &DirectInlineCandidateContextV2,
    collateral: &DirectInlineCollateralFrameV2,
    settlement: &InlineOrdinarySettlementV2,
) -> Result<CustodyCompilationV2> {
    // The fee destination does not move in this transaction. It still tracks
    // the seller's when a market routes both to one account -- an alias
    // `validate_collateral` permits only when every observed field agrees.
    let aliased = collateral.seller_destination.account == collateral.fee_destination.account;
    let count = custody_transfer_count(settlement)?;
    if count == 0 {
        return Ok(CustodyCompilationV2 {
            count: 0,
            source_after: collateral.buyer_source.balance,
            delegated_after: collateral.buyer_source.delegated_amount,
            seller_after: collateral.seller_destination.balance,
            fee_after: if aliased {
                collateral.seller_destination.balance
            } else {
                collateral.fee_destination.balance
            },
        });
    }
    let effect = compile_custody_effect(
        direct,
        context,
        &collateral.buyer_source,
        &collateral.seller_destination,
        settlement.effects.seller_net_collateral_credit,
        collateral.buyer_source.balance,
        collateral.buyer_source.delegated_amount,
        collateral.seller_destination.balance,
        settlement.effects.total_fee_transfer,
        settlement.effects.buyer_collateral_debit,
    )?;
    // The tx1 closing postconditions. The buyer is debited the seller's net and
    // nothing more, the allowance keeps exactly the unsettled `combined_fee`,
    // and the fee destination does not move in this transaction at all.
    if effect.source_after
        != collateral
            .buyer_source
            .balance
            .checked_sub(settlement.effects.seller_net_collateral_credit)
            .ok_or(DirectInlineCandidateErrorV2::Arithmetic)?
        || effect.delegated_after
            != collateral
                .buyer_source
                .delegated_amount
                .checked_sub(settlement.effects.seller_net_collateral_credit)
                .ok_or(DirectInlineCandidateErrorV2::Arithmetic)?
        || effect.delegated_after != settlement.effects.total_fee_transfer
    {
        return Err(DirectInlineCandidateErrorV2::Postcondition);
    }
    Ok(CustodyCompilationV2 {
        count: 1,
        source_after: effect.source_after,
        delegated_after: effect.delegated_after,
        seller_after: effect.destination_after,
        fee_after: if aliased {
            effect.destination_after
        } else {
            collateral.fee_destination.balance
        },
    })
}

/// How many Custody transfers this transaction dispatches, and the retirement.
///
/// The seller leg runs whenever the seller nets anything; the fee leg never
/// runs here. A settlement that nets the seller nothing while still owing a fee
/// is the retired `FeeSole` shape and is refused by name.
fn custody_transfer_count(settlement: &InlineOrdinarySettlementV2) -> Result<u8> {
    match (
        settlement.effects.seller_net_collateral_credit != 0,
        settlement.effects.total_fee_transfer != 0,
    ) {
        (false, true) => Err(DirectInlineCandidateErrorV2::FeeSoleRetired),
        (false, false) => Ok(0),
        (true, _) => Ok(1),
    }
}

/// Project the one Custody effect this transaction dispatches.
///
/// `transfer_index` addresses only dispatched routes, of which there is now at
/// most one: the seller-net leg. The fee leg is a second transaction and the
/// fee-only route is retired, so any other index is a width refusal.
#[inline(never)]
pub fn project_inline_custody_effect_v2(
    direct: InlineOrdinaryInputV2,
    context: DirectInlineCandidateContextV2,
    collateral: DirectInlineCollateralFrameV2,
    settlement: InlineOrdinarySettlementV2,
    transfer_index: u8,
) -> Result<DirectInlineCustodyEffectV2> {
    validate_context(direct, context)?;
    let authenticated =
        settle_inline_ordinary_v2(direct).map_err(|_| DirectInlineCandidateErrorV2::Settlement)?;
    if authenticated != settlement {
        return Err(DirectInlineCandidateErrorV2::Binding);
    }
    validate_collateral(direct, context, collateral, settlement)?;
    if usize::from(transfer_index) >= usize::from(custody_transfer_count(&settlement)?) {
        return Err(DirectInlineCandidateErrorV2::Width);
    }
    compile_custody_effect(
        &direct,
        &context,
        &collateral.buyer_source,
        &collateral.seller_destination,
        settlement.effects.seller_net_collateral_credit,
        collateral.buyer_source.balance,
        collateral.buyer_source.delegated_amount,
        collateral.seller_destination.balance,
        settlement.effects.total_fee_transfer,
        settlement.effects.buyer_collateral_debit,
    )
}

/// Differentially verify the authenticated Effect request/dispatch partition.
///
/// Claims and every enabled Custody slice are compared byte-for-byte to the
/// shared typed candidate. The four Effect-owned Custody slots are exhaustive,
/// disjoint, and ordered: seller terminal; seller intermediate; fee
/// continuation; fee sole. Inactive raw bytes remain data-defined Effect
/// authority and are deliberately never decoded or used in arithmetic.
#[inline(never)]
pub fn prepare_and_verify_inline_effect_partition_v2(
    direct: InlineOrdinaryInputV2,
    context: DirectInlineCandidateContextV2,
    collateral: DirectInlineCollateralFrameV2,
    request_bank: &[u8],
    dispatch: DirectInlineEffectDispatchV2,
) -> Result<DirectInlineCandidateV2> {
    let candidate = prepare_inline_ordinary_candidate_v2(direct, context, collateral)?;
    verify_inline_effect_partition_prepared_v2(
        direct,
        context,
        collateral,
        candidate,
        request_bank,
        dispatch,
    )?;
    Ok(candidate)
}

/// Hostile-check a separately supplied candidate against the same exact
/// Effect partition. Callers that need both should prefer
/// [`prepare_and_verify_inline_effect_partition_v2`], which cannot substitute
/// a candidate between preparation and the partition join.
#[inline(never)]
pub fn verify_inline_effect_partition_v2(
    direct: InlineOrdinaryInputV2,
    context: DirectInlineCandidateContextV2,
    collateral: DirectInlineCollateralFrameV2,
    candidate: DirectInlineCandidateV2,
    request_bank: &[u8],
    dispatch: DirectInlineEffectDispatchV2,
) -> Result<()> {
    let authenticated = prepare_and_verify_inline_effect_partition_v2(
        direct,
        context,
        collateral,
        request_bank,
        dispatch,
    )?;
    if authenticated != candidate {
        return Err(DirectInlineCandidateErrorV2::Postcondition);
    }
    Ok(())
}

#[inline(never)]
fn verify_inline_effect_partition_prepared_v2(
    direct: InlineOrdinaryInputV2,
    context: DirectInlineCandidateContextV2,
    collateral: DirectInlineCollateralFrameV2,
    candidate: DirectInlineCandidateV2,
    request_bank: &[u8],
    dispatch: DirectInlineEffectDispatchV2,
) -> Result<()> {
    if request_bank.len() != DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3 {
        return Err(DirectInlineCandidateErrorV2::Width);
    }
    let claims = encode_inline_claims_request_v2(direct, context)?;
    if request_bank.get(..DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2) != Some(claims.as_slice()) {
        return Err(DirectInlineCandidateErrorV2::Postcondition);
    }
    let expected = expected_custody_slots(candidate.settlement)?;
    let count = usize::from(dispatch.custody_count);
    if count != usize::from(candidate.custody_count)
        || count > DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2
        || dispatch.custody_slots.get(..count) != expected.get(..count)
    {
        return Err(DirectInlineCandidateErrorV2::Postcondition);
    }
    let mut seen = [false; DIRECT_INLINE_CUSTODY_ROUTE_SLOTS_V2];
    let mut source_after = collateral.buyer_source.balance;
    let mut delegated_after = collateral.buyer_source.delegated_amount;
    let mut seller_after = collateral.seller_destination.balance;
    let aliased = collateral.seller_destination.account == collateral.fee_destination.account;
    if count == 1 {
        let slot = usize::from(
            *dispatch
                .custody_slots
                .first()
                .ok_or(DirectInlineCandidateErrorV2::Width)?,
        );
        *seen
            .get_mut(slot)
            .ok_or(DirectInlineCandidateErrorV2::Width)? = true;
        let effect = compile_custody_effect(
            &direct,
            &context,
            &collateral.buyer_source,
            &collateral.seller_destination,
            candidate.settlement.effects.seller_net_collateral_credit,
            source_after,
            delegated_after,
            seller_after,
            candidate.settlement.effects.total_fee_transfer,
            candidate.settlement.effects.buyer_collateral_debit,
        )?;
        let expected_request = effect
            .request
            .encode()
            .map_err(|_| DirectInlineCandidateErrorV2::Custody)?;
        let start = DIRECT_INLINE_CLAIMS_REQUEST_BYTES_V2
            .checked_add(
                slot.checked_mul(DELEGATED_CUSTODY_REQUEST_BYTES_V2)
                    .ok_or(DirectInlineCandidateErrorV2::Arithmetic)?,
            )
            .ok_or(DirectInlineCandidateErrorV2::Arithmetic)?;
        let end = start
            .checked_add(DELEGATED_CUSTODY_REQUEST_BYTES_V2)
            .ok_or(DirectInlineCandidateErrorV2::Arithmetic)?;
        if request_bank.get(start..end) != Some(expected_request.as_slice()) {
            return Err(DirectInlineCandidateErrorV2::Postcondition);
        }
        source_after = effect.source_after;
        delegated_after = effect.delegated_after;
        seller_after = effect.destination_after;
    }
    let fee_after = if aliased {
        seller_after
    } else {
        collateral.fee_destination.balance
    };
    if dispatch.child_dispatch_writable != seen
        || source_after != candidate.buyer_source_after
        || delegated_after != candidate.buyer_delegated_after
        || seller_after != candidate.seller_destination_after
        || fee_after != candidate.fee_destination_after
    {
        return Err(DirectInlineCandidateErrorV2::Postcondition);
    }
    Ok(())
}

fn expected_custody_slots(
    settlement: InlineOrdinarySettlementV2,
) -> Result<[u8; DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2]> {
    // Slot 0 is the terminal seller-only route, slot 1 the non-terminal one
    // that leaves the fee's allowance standing. Slot 2 (fee continuation) is
    // the second transaction's, and slot 3 (`FeeSole`) is retired -- both are
    // unreachable from here, and the count refuses the shapes that would want
    // them.
    if custody_transfer_count(&settlement)? == 0 {
        return Ok([0]);
    }
    let terminal = settlement.effects.total_fee_transfer == 0;
    Ok(if terminal { [0] } else { [1] })
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn compile_custody_effect(
    direct: &InlineOrdinaryInputV2,
    context: &DirectInlineCandidateContextV2,
    source: &DirectExternalDebitV2,
    destination: &DirectExternalCollateralV2,
    amount: u64,
    source_before: u64,
    delegated_before: u64,
    destination_before: u64,
    fee_still_owed: u64,
    total_debit: u64,
) -> Result<DirectInlineCustodyEffectV2> {
    let source_after = source_before
        .checked_sub(amount)
        .ok_or(DirectInlineCandidateErrorV2::Arithmetic)?;
    let delegated_after = delegated_before
        .checked_sub(amount)
        .ok_or(DirectInlineCandidateErrorV2::Arithmetic)?;
    let destination_after = destination_before
        .checked_add(amount)
        .ok_or(DirectInlineCandidateErrorV2::Arithmetic)?;
    let custody = custody_request(direct, context, source, destination, 0, amount)?;
    // The one change of substance in tx1, and the reason the seller leg keeps
    // its shipped `SellerIntermediate` shape: terminality now follows the FEE,
    // not the transfer count. A fee-bearing fill leaves the delegation standing
    // at exactly `combined_fee` for the second transaction to spend; a zero-fee
    // fill closes it, exactly as it always did. `DelegatedCustodyRequestV2`'s
    // own `terminal == (allowance_after == 0)` relation is what keeps the two
    // statements from drifting apart.
    let terminal = fee_still_owed == 0;
    let request = DelegatedCustodyRequestV2 {
        custody,
        starts_atomic_debit: true,
        terminal,
        delegate_before: context.custody_authority,
        delegate_after: if terminal {
            [0; 32]
        } else {
            context.custody_authority
        },
        total_debit,
        allowance_before: delegated_before,
        allowance_after: delegated_after,
    };
    request
        .validate()
        .map_err(|_| DirectInlineCandidateErrorV2::Custody)?;
    Ok(DirectInlineCustodyEffectV2 {
        request,
        source_after,
        delegated_after,
        destination_after,
    })
}

#[inline(never)]
fn custody_request(
    direct: &InlineOrdinaryInputV2,
    context: &DirectInlineCandidateContextV2,
    source: &DirectExternalDebitV2,
    destination: &DirectExternalCollateralV2,
    transfer_index: usize,
    amount: u64,
) -> Result<CustodyRequestV1> {
    let buyer_intent = direct.buyer.authenticated.intent();
    let expected_revision = context
        .custody_revision
        .checked_add(
            u64::try_from(transfer_index).map_err(|_| DirectInlineCandidateErrorV2::Arithmetic)?,
        )
        .ok_or(DirectInlineCandidateErrorV2::Arithmetic)?;
    let mut request = base_custody_request(context, transfer_index, amount)?;
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
        .map_err(|_| DirectInlineCandidateErrorV2::Custody)?;
    Ok(request)
}

#[inline(never)]
fn base_custody_request(
    context: &DirectInlineCandidateContextV2,
    transfer_index: usize,
    amount: u64,
) -> Result<CustodyRequestV1> {
    Ok(CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::External,
        destination_compartment: CompartmentV1::External,
        release_set: context.release_set,
        market: context.market,
        realm: context.realm,
        context: context.buyer_maker_root,
        caller_program: context.trading_program,
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [1; 32],
            destination_owner: [2; 32],
            order: context.parent_request_digest,
            parent_request_digest: context.parent_request_digest,
            order_nonce: 0,
            generation: context.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: u16::try_from(transfer_index)
                .map_err(|_| DirectInlineCandidateErrorV2::Arithmetic)?,
        },
        source: [1; 32],
        destination: [2; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: context.mint,
        token_program: context.token_program,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: context.custody_revision,
        resulting_revision: checked_next(context.custody_revision)?,
        amount,
        rent_lamports: 0,
    })
}

fn checked_next(value: u64) -> Result<u64> {
    value
        .checked_add(1)
        .ok_or(DirectInlineCandidateErrorV2::Arithmetic)
}

struct CustodyCompilationV2 {
    count: u8,
    source_after: u64,
    delegated_after: u64,
    seller_after: u64,
    fee_after: u64,
}
