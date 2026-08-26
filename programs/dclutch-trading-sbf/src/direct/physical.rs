//! Ordinary Direct successor child requests and postcondition verification.
//!
//! This module is not a second transition or effect authority. It calls the
//! Direct successor's sole checked settlement, projects that accepted result
//! into the canonical Claims request and verifies its immediate
//! acknowledgement. Record-keyed Buy custody is owned by `buy_escrow`. The
//! composing Trading outer authenticates the descriptor, account profile,
//! fixed-role programs, accounts, and receipt producers, then commits Direct
//! state last.

use dclutch_claims_svm::{
    CLAIMS_PLAN_HEADER_BYTES_V1, CallerRole as ClaimsCallerRole, ClaimsAction, ClaimsPlanV1,
    ClaimsReceiptV1,
};
use dclutch_direct_codec::successor::{
    DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_REGISTERED_RECORD_BYTES_V2, DirectExecutionConfigV1,
    RegisteredOrdinaryInputV2, RegisteredOrdinarySettlementV2, RegisteredRecordAfterFillV2,
    RegisteredRecordCloseV2, settle_registered_ordinary_v2,
};
use dclutch_market_core_codec::{CoreMarketViewV1, Phase};
use solana_program::hash::hash;

/// Stable refusal from ordinary Direct physical projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectPhysicalError {
    /// A required program, content, account, or digest identity was zero.
    ZeroIdentity,
    /// Market, generation, maker, config, or endpoint bindings differed.
    Binding,
    /// Runtime Product width or caller-owned buffers had another exact width.
    Width,
    /// Checked balance, revision, or vector arithmetic failed.
    Arithmetic,
    /// The sole Direct successor settlement refused.
    Settlement,
    /// Canonical Claims request construction or receipt verification refused.
    Claims,
    /// Canonical Custody request construction or receipt verification refused.
    Custody,
    /// A child receipt or observed poststate differed from the exact plan.
    Postcondition,
    /// Exact Direct state-candidate encoding or output geometry refused.
    State,
}

/// Result alias for Direct physical planning.
pub type Result<T> = core::result::Result<T, DirectPhysicalError>;

/// One authenticated external collateral token account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectExternalCollateralV2 {
    /// Exact token-account key.
    pub account: [u8; 32],
    /// Exact persisted external token authority.
    pub owner: [u8; 32],
    /// Authenticated token amount before this outer action.
    pub balance: u64,
}

/// One authenticated external collateral source delegated to Custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectExternalDebitV2 {
    /// Exact token-account key.
    pub account: [u8; 32],
    /// Exact persisted external token authority.
    pub owner: [u8; 32],
    /// Canonical Custody transfer-authority PDA.
    pub delegate: [u8; 32],
    /// Exact remaining delegated allowance before this fill.
    pub delegated_amount: u64,
    /// Authenticated token amount before this fill.
    pub balance: u64,
}

/// Authenticated fixed-role and revision facts for one ordinary Claims fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectOrdinaryClaimsContextV2 {
    /// Sparse-Core view after exact Market, Product, Realm, and Registry authentication.
    pub core_market: CoreMarketViewV1,
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// Current Registry-selected Claims program.
    pub claims_program: [u8; 32],
    /// SHA-256 of the complete canonical parent Trading request.
    pub parent_request_digest: [u8; 32],
    /// Claims aggregate revision before this fill.
    pub claims_market_revision: u64,
    /// Seller Claims Position revision before this fill.
    pub seller_position_revision: u64,
    /// Buyer Claims Position revision before this fill.
    pub buyer_position_revision: u64,
}

impl DirectOrdinaryClaimsContextV2 {
    fn validate(self) -> Result<()> {
        if self.trading_program == [0; 32]
            || self.claims_program == [0; 32]
            || self.parent_request_digest == [0; 32]
            || self.core_market.phase() != Phase::Open
            || self.core_market.release_set().bindings[1]
                .program
                .to_bytes()
                != self.claims_program
            || self.core_market.release_set().bindings[2]
                .program
                .to_bytes()
                != self.trading_program
        {
            return Err(DirectPhysicalError::Binding);
        }
        Ok(())
    }
}

/// Complete ordinary Claims candidate. Direct state remains unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectOrdinaryClaimsPlanV2 {
    /// Sole accepted Direct settlement candidate.
    pub settlement: RegisteredOrdinarySettlementV2,
    /// Exact encoded runtime-width Claims packet length.
    pub claims_bytes: usize,
}

/// Terminal disposition of one registered-record account after child success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRecordCommitV2 {
    /// Persist the encoded live record from the corresponding output buffer.
    WriteLive,
    /// Close the record and route exact rent plus donation as described.
    Close(RegisteredRecordCloseV2),
}

/// Direct state candidate encoded only after the semantic settlement accepts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectOrdinaryStateCandidateV2 {
    /// Seller record disposition.
    pub seller_record: DirectRecordCommitV2,
    /// Buyer record disposition.
    pub buyer_record: DirectRecordCommitV2,
}

/// Caller-owned scratch and output buffers for commit-last Direct state.
///
/// Scratch may change on refusal. Every output is byte-for-byte unchanged on
/// refusal. For a closed record its output remains unchanged on success too;
/// the Trading outer closes that account only after all child receipts accept.
pub struct DirectOrdinaryStateBuffersV2<'a> {
    /// Seller maker-root encoded output.
    pub seller_maker_output: &'a mut [u8],
    /// Buyer maker-root encoded output.
    pub buyer_maker_output: &'a mut [u8],
    /// Seller live-record encoding scratch.
    pub seller_record_scratch: &'a mut [u8],
    /// Buyer live-record encoding scratch.
    pub buyer_record_scratch: &'a mut [u8],
    /// Seller live-record encoded output.
    pub seller_record_output: &'a mut [u8],
    /// Buyer live-record encoded output.
    pub buyer_record_output: &'a mut [u8],
}

/// Encode the complete ordinary Direct state candidate without committing it.
///
/// The common Trading outer invokes Claims and every positive Custody request,
/// verifies their immediate producer/poststate receipts, then copies these
/// maker/record outputs or performs the exact record closes as its final local
/// effects. This function never writes authoritative accounts itself.
pub fn encode_registered_ordinary_state_candidate_v2(
    settlement: RegisteredOrdinarySettlementV2,
    config: DirectExecutionConfigV1,
    outcome_count: u32,
    buffers: DirectOrdinaryStateBuffersV2<'_>,
) -> Result<DirectOrdinaryStateCandidateV2> {
    if buffers.seller_maker_output.len() != DIRECT_MAKER_REPLAY_BYTES_V1
        || buffers.buyer_maker_output.len() != DIRECT_MAKER_REPLAY_BYTES_V1
        || buffers.seller_record_scratch.len() != DIRECT_REGISTERED_RECORD_BYTES_V2
        || buffers.buyer_record_scratch.len() != DIRECT_REGISTERED_RECORD_BYTES_V2
        || buffers.seller_record_output.len() != DIRECT_REGISTERED_RECORD_BYTES_V2
        || buffers.buyer_record_output.len() != DIRECT_REGISTERED_RECORD_BYTES_V2
    {
        return Err(DirectPhysicalError::Width);
    }
    let seller_maker = settlement
        .seller
        .maker_root
        .encode()
        .map_err(|_| DirectPhysicalError::State)?;
    let buyer_maker = settlement
        .buyer
        .maker_root
        .encode()
        .map_err(|_| DirectPhysicalError::State)?;
    let seller_record = encode_record_candidate(
        settlement.seller.record,
        config,
        outcome_count,
        buffers.seller_record_scratch,
    )?;
    let buyer_record = encode_record_candidate(
        settlement.buyer.record,
        config,
        outcome_count,
        buffers.buyer_record_scratch,
    )?;

    buffers.seller_maker_output.copy_from_slice(&seller_maker);
    buffers.buyer_maker_output.copy_from_slice(&buyer_maker);
    if seller_record == DirectRecordCommitV2::WriteLive {
        buffers
            .seller_record_output
            .copy_from_slice(buffers.seller_record_scratch);
    }
    if buyer_record == DirectRecordCommitV2::WriteLive {
        buffers
            .buyer_record_output
            .copy_from_slice(buffers.buyer_record_scratch);
    }
    Ok(DirectOrdinaryStateCandidateV2 {
        seller_record,
        buyer_record,
    })
}

/// Preview one ordinary successor fill and construct its exact child requests.
///
/// `claims_output` is unchanged on every refusal. The two scratch buffers may
/// contain a rejected candidate. Their widths are derived only from the
/// authenticated Product `outcome_count`; this function has no protocol width
/// ceiling.
#[allow(clippy::too_many_arguments)]
pub fn prepare_registered_ordinary_claims_v2(
    input: RegisteredOrdinaryInputV2,
    context: DirectOrdinaryClaimsContextV2,
    quantity_scratch: &mut [u8],
    claims_scratch: &mut [u8],
    claims_output: &mut [u8],
) -> Result<DirectOrdinaryClaimsPlanV2> {
    context.validate()?;
    let seller_record = input.seller.record;
    let buyer_record = input.buyer.record;
    authenticate_claims_bindings(input, context)?;
    let settlement =
        settle_registered_ordinary_v2(input).map_err(|_| DirectPhysicalError::Settlement)?;

    let outcome_count = context.core_market.product().outcome_count;
    let tail_bytes = usize::try_from(outcome_count)
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

    quantity_scratch.fill(0);
    let quantity_offset = usize::try_from(seller_record.intent().outcome)
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
        .copy_from_slice(&input.execution.fill.to_le_bytes());
    let claims = ClaimsPlanV1::new(
        ClaimsAction::TransferNative,
        ClaimsCallerRole::Trading,
        context.core_market.release_set().release_set_id.to_bytes(),
        context.core_market.market().to_bytes(),
        context.parent_request_digest,
        seller_record.maker(),
        buyer_record.maker(),
        context.claims_market_revision,
        context.seller_position_revision,
        context.buyer_position_revision,
        outcome_count,
        quantity_scratch,
    )
    .map_err(|_| DirectPhysicalError::Claims)?;
    claims
        .encode_into(claims_scratch)
        .map_err(|_| DirectPhysicalError::Claims)?;
    ClaimsPlanV1::decode(claims_scratch).map_err(|_| DirectPhysicalError::Claims)?;
    claims_output.copy_from_slice(claims_scratch);

    Ok(DirectOrdinaryClaimsPlanV2 {
        settlement,
        claims_bytes,
    })
}

/// Verify one immediate Claims acknowledgement against the exact packet.
pub fn verify_direct_claims_receipt_v2(
    context: DirectOrdinaryClaimsContextV2,
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
        || receipt.post_market_revision()
            != context
                .claims_market_revision
                .checked_add(1)
                .ok_or(DirectPhysicalError::Arithmetic)?
        || receipt.post_source_revision()
            != context
                .seller_position_revision
                .checked_add(1)
                .ok_or(DirectPhysicalError::Arithmetic)?
        || receipt.post_destination_revision()
            != context
                .buyer_position_revision
                .checked_add(1)
                .ok_or(DirectPhysicalError::Arithmetic)?
        || receipt.payout() != 0
        || plan.source_owner() == plan.destination_owner()
    {
        return Err(DirectPhysicalError::Postcondition);
    }
    Ok(())
}

fn authenticate_claims_bindings(
    input: RegisteredOrdinaryInputV2,
    context: DirectOrdinaryClaimsContextV2,
) -> Result<()> {
    let seller = input.seller.record;
    let buyer = input.buyer.record;
    let seller_intent = seller.intent();
    let buyer_intent = buyer.intent();
    if context.core_market.market().to_bytes() != seller_intent.market
        || context.core_market.market().to_bytes() != buyer_intent.market
        || context.core_market.generation() != seller_intent.generation
        || context.core_market.generation() != buyer_intent.generation
        || context.core_market.product().outcome_count != input.execution.outcome_count
        || seller.maker() == buyer.maker()
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn encode_record_candidate(
    candidate: RegisteredRecordAfterFillV2,
    config: DirectExecutionConfigV1,
    outcome_count: u32,
    scratch: &mut [u8],
) -> Result<DirectRecordCommitV2> {
    match candidate {
        RegisteredRecordAfterFillV2::Live(record) => {
            let encoded = record
                .encode_selected(config, outcome_count)
                .map_err(|_| DirectPhysicalError::State)?;
            scratch.copy_from_slice(&encoded);
            Ok(DirectRecordCommitV2::WriteLive)
        }
        RegisteredRecordAfterFillV2::Closed(close) => Ok(DirectRecordCommitV2::Close(close)),
    }
}
