//! Ordinary Direct successor child requests and postcondition verification.
//!
//! This module is not a second transition or effect authority. It calls the
//! Direct successor's sole checked settlement, projects that accepted result
//! into canonical Claims and Custody requests, and verifies immediate child
//! acknowledgements. The composing Trading outer authenticates the descriptor,
//! account profile, fixed-role programs, accounts, and receipt producers, then
//! commits Direct state last.

use dclutch_claims_svm::{
    CLAIMS_PLAN_HEADER_BYTES_V1, CallerRole as ClaimsCallerRole, ClaimsAction, ClaimsPlanV1,
    ClaimsReceiptV1,
};
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyReceiptV1, CustodyRequestV1, OperationV1,
};
use dclutch_direct_codec::successor::{
    DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_REGISTERED_RECORD_BYTES_V2, DirectExecutionConfigV1,
    DirectRegisteredIntentV2, RegisteredOrdinaryInputV2, RegisteredOrdinarySettlementV2,
    RegisteredRecordAfterFillV2, RegisteredRecordCloseV2, settle_registered_ordinary_v2,
};
use solana_program::hash::hash;

/// Maximum number of collateral transfers emitted by one ordinary match.
pub const DIRECT_ORDINARY_CUSTODY_EFFECT_CAPACITY_V2: usize = 2;

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

impl DirectExternalCollateralV2 {
    fn validate(self) -> Result<()> {
        if is_zero(self.account) || is_zero(self.owner) {
            Err(DirectPhysicalError::ZeroIdentity)
        } else {
            Ok(())
        }
    }
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

impl DirectExternalDebitV2 {
    fn validate(self) -> Result<()> {
        if is_zero(self.account) || is_zero(self.owner) || is_zero(self.delegate) {
            Err(DirectPhysicalError::ZeroIdentity)
        } else {
            Ok(())
        }
    }
}

/// Exact ordinary collateral-account frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectOrdinaryCollateralFrameV2 {
    /// Buyer-signed external source.
    pub buyer_source: DirectExternalDebitV2,
    /// Seller-signed external destination.
    pub seller_destination: DirectExternalCollateralV2,
    /// Immutable config-recipient external destination.
    pub fee_destination: DirectExternalCollateralV2,
}

impl DirectOrdinaryCollateralFrameV2 {
    fn validate(self) -> Result<()> {
        self.buyer_source.validate()?;
        self.seller_destination.validate()?;
        self.fee_destination.validate()?;
        if self.buyer_source.account == self.seller_destination.account
            || self.buyer_source.account == self.fee_destination.account
        {
            return Err(DirectPhysicalError::Binding);
        }
        if self.seller_destination.account == self.fee_destination.account
            && self.seller_destination != self.fee_destination
        {
            return Err(DirectPhysicalError::Binding);
        }
        Ok(())
    }
}

/// Authenticated fixed-role and revision facts for one ordinary fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectOrdinaryPhysicalContextV2 {
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// Current Registry-selected Claims program.
    pub claims_program: [u8; 32],
    /// Current Registry-selected Custody program.
    pub custody_program: [u8; 32],
    /// Canonical Custody transfer-authority PDA for this Market/release set.
    pub custody_authority: [u8; 32],
    /// Immutable execution-release-set content identity.
    pub release_set: [u8; 32],
    /// Exact Core Market.
    pub market: [u8; 32],
    /// Exact immutable Realm content identity.
    pub realm: [u8; 32],
    /// Realm-selected collateral Mint.
    pub mint: [u8; 32],
    /// Realm-selected Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// SHA-256 of the complete canonical parent Trading request.
    pub parent_request_digest: [u8; 32],
    /// Buyer maker-root key and per-maker Custody replay context.
    pub buyer_maker_root: [u8; 32],
    /// Exact buyer registered-record key bound as the Custody order coordinate.
    pub buyer_record: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Claims aggregate revision before this fill.
    pub claims_market_revision: u64,
    /// Seller Claims Position revision before this fill.
    pub seller_position_revision: u64,
    /// Buyer Claims Position revision before this fill.
    pub buyer_position_revision: u64,
    /// Per-buyer-maker Custody replay revision before the first emitted transfer.
    pub custody_replay_revision: u64,
}

impl DirectOrdinaryPhysicalContextV2 {
    fn validate(self) -> Result<()> {
        for identity in [
            self.trading_program,
            self.claims_program,
            self.custody_program,
            self.custody_authority,
            self.release_set,
            self.market,
            self.realm,
            self.mint,
            self.token_program,
            self.parent_request_digest,
            self.buyer_maker_root,
            self.buyer_record,
        ] {
            if is_zero(identity) {
                return Err(DirectPhysicalError::ZeroIdentity);
            }
        }
        Ok(())
    }
}

/// One expected Custody transfer and its exact post-balances.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCustodyEffectV2 {
    /// Canonical distinct-owner Custody request.
    pub request: CustodyRequestV1,
    /// Expected source balance after this transfer.
    pub source_after: u64,
    /// Expected destination balance after this transfer.
    pub destination_after: u64,
}

/// Complete physical candidate. Direct state remains unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectOrdinaryPhysicalPlanV2 {
    /// Sole accepted Direct settlement candidate.
    pub settlement: RegisteredOrdinarySettlementV2,
    /// Number of emitted positive Custody effects in canonical order.
    pub custody_count: u8,
    /// Seller-net then combined-fee requests; zero transfers are omitted.
    pub custody: [Option<DirectCustodyEffectV2>; DIRECT_ORDINARY_CUSTODY_EFFECT_CAPACITY_V2],
    /// Exact encoded runtime-width Claims packet length.
    pub claims_bytes: usize,
    /// Buyer source balance after every emitted transfer.
    pub buyer_source_after: u64,
    /// Buyer Custody-delegate allowance after every emitted transfer.
    ///
    /// A terminal price-improved fill can leave a positive residual. It grants
    /// no new Direct nonce authority, but the maker should revoke it once the
    /// registered record closes.
    pub buyer_delegated_after: u64,
    /// Seller destination balance after every emitted transfer.
    pub seller_destination_after: u64,
    /// Fee destination balance after every emitted transfer.
    pub fee_destination_after: u64,
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
pub fn prepare_registered_ordinary_physical_v2(
    input: RegisteredOrdinaryInputV2,
    context: DirectOrdinaryPhysicalContextV2,
    collateral: DirectOrdinaryCollateralFrameV2,
    quantity_scratch: &mut [u8],
    claims_scratch: &mut [u8],
    claims_output: &mut [u8],
) -> Result<DirectOrdinaryPhysicalPlanV2> {
    context.validate()?;
    collateral.validate()?;
    let seller_record = input.seller.record;
    let buyer_record = input.buyer.record;
    authenticate_bindings(input, context, collateral)?;
    let settlement =
        settle_registered_ordinary_v2(input).map_err(|_| DirectPhysicalError::Settlement)?;

    let tail_bytes = usize::try_from(input.execution.outcome_count)
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

    let custody = compile_custody(context, collateral, buyer_record, settlement)?;

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
        context.release_set,
        context.market,
        context.parent_request_digest,
        seller_record.maker(),
        buyer_record.maker(),
        context.claims_market_revision,
        context.seller_position_revision,
        context.buyer_position_revision,
        input.execution.outcome_count,
        quantity_scratch,
    )
    .map_err(|_| DirectPhysicalError::Claims)?;
    claims
        .encode_into(claims_scratch)
        .map_err(|_| DirectPhysicalError::Claims)?;
    ClaimsPlanV1::decode(claims_scratch).map_err(|_| DirectPhysicalError::Claims)?;
    claims_output.copy_from_slice(claims_scratch);

    Ok(DirectOrdinaryPhysicalPlanV2 {
        settlement,
        custody_count: custody.count,
        custody: custody.effects,
        claims_bytes,
        buyer_source_after: custody.source_after,
        buyer_delegated_after: custody.delegated_after,
        seller_destination_after: custody.seller_after,
        fee_destination_after: custody.fee_after,
    })
}

/// Verify one immediate Claims acknowledgement against the exact packet.
pub fn verify_direct_claims_receipt_v2(
    context: DirectOrdinaryPhysicalContextV2,
    claims_packet: &[u8],
    receipt_bytes: &[u8],
) -> Result<()> {
    let plan = ClaimsPlanV1::decode(claims_packet).map_err(|_| DirectPhysicalError::Claims)?;
    let receipt =
        ClaimsReceiptV1::decode(receipt_bytes).map_err(|_| DirectPhysicalError::Claims)?;
    if receipt.caller_role() != ClaimsCallerRole::Trading
        || receipt.action() != ClaimsAction::TransferNative
        || receipt.release_set_id() != context.release_set
        || receipt.market() != context.market
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

/// Verify one immediate Custody acknowledgement and exact token deltas.
pub fn verify_direct_custody_receipt_v2(
    effect: DirectCustodyEffectV2,
    receipt_bytes: &[u8],
    poststate_commitment: [u8; 32],
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
            poststate_commitment,
        )
        .map_err(|_| DirectPhysicalError::Custody)?;
    if receipt.evidence.source_after != effect.source_after
        || receipt.evidence.destination_after != effect.destination_after
    {
        return Err(DirectPhysicalError::Postcondition);
    }
    Ok(())
}

fn authenticate_bindings(
    input: RegisteredOrdinaryInputV2,
    context: DirectOrdinaryPhysicalContextV2,
    collateral: DirectOrdinaryCollateralFrameV2,
) -> Result<()> {
    let seller = input.seller.record;
    let buyer = input.buyer.record;
    let seller_intent = seller.intent();
    let buyer_intent = buyer.intent();
    if context.market != seller_intent.market
        || context.market != buyer_intent.market
        || context.generation != seller_intent.generation
        || context.generation != buyer_intent.generation
        || collateral.buyer_source.account != buyer_intent.collateral_account
        || collateral.buyer_source.owner != buyer.maker()
        || collateral.buyer_source.delegate != context.custody_authority
        || collateral.buyer_source.delegated_amount != buyer.reserved_collateral()
        || collateral.seller_destination.account != seller_intent.collateral_account
        || collateral.seller_destination.owner != seller.maker()
        || collateral.fee_destination.owner != input.execution.config.fee_recipient()
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn compile_custody(
    context: DirectOrdinaryPhysicalContextV2,
    collateral: DirectOrdinaryCollateralFrameV2,
    buyer_record: DirectRegisteredIntentV2,
    settlement: RegisteredOrdinarySettlementV2,
) -> Result<CustodyCompilationV2> {
    if collateral.buyer_source.balance < settlement.buyer_collateral_debit {
        return Err(DirectPhysicalError::Arithmetic);
    }
    let mut effects = [None; DIRECT_ORDINARY_CUSTODY_EFFECT_CAPACITY_V2];
    let mut count = 0_usize;
    let mut source_balance = collateral.buyer_source.balance;
    let mut delegated_amount = collateral.buyer_source.delegated_amount;
    let mut seller_balance = collateral.seller_destination.balance;
    let mut fee_balance =
        if collateral.seller_destination.account == collateral.fee_destination.account {
            seller_balance
        } else {
            collateral.fee_destination.balance
        };

    if settlement.seller_net_collateral_credit != 0 {
        let source_after = source_balance
            .checked_sub(settlement.seller_net_collateral_credit)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        let destination_after = seller_balance
            .checked_add(settlement.seller_net_collateral_credit)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        let request = custody_request(
            context,
            buyer_record,
            collateral.buyer_source,
            collateral.seller_destination,
            count,
            settlement.seller_net_collateral_credit,
        )?;
        *effects
            .get_mut(count)
            .ok_or(DirectPhysicalError::Arithmetic)? = Some(DirectCustodyEffectV2 {
            request,
            source_after,
            destination_after,
        });
        count = count
            .checked_add(1)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        source_balance = source_after;
        delegated_amount = delegated_amount
            .checked_sub(settlement.seller_net_collateral_credit)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        seller_balance = destination_after;
        if collateral.seller_destination.account == collateral.fee_destination.account {
            fee_balance = destination_after;
        }
    }

    if settlement.total_fee_transfer != 0 {
        let source_after = source_balance
            .checked_sub(settlement.total_fee_transfer)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        let destination_after = fee_balance
            .checked_add(settlement.total_fee_transfer)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        let request = custody_request(
            context,
            buyer_record,
            collateral.buyer_source,
            collateral.fee_destination,
            count,
            settlement.total_fee_transfer,
        )?;
        *effects
            .get_mut(count)
            .ok_or(DirectPhysicalError::Arithmetic)? = Some(DirectCustodyEffectV2 {
            request,
            source_after,
            destination_after,
        });
        count = count
            .checked_add(1)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        source_balance = source_after;
        delegated_amount = delegated_amount
            .checked_sub(settlement.total_fee_transfer)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        fee_balance = destination_after;
        if collateral.seller_destination.account == collateral.fee_destination.account {
            seller_balance = destination_after;
        }
    }

    let expected_residual = match settlement.buyer.record {
        RegisteredRecordAfterFillV2::Live(record) => record.reserved_collateral(),
        RegisteredRecordAfterFillV2::Closed(close) => close.collateral_refund,
    };
    if source_balance
        != collateral
            .buyer_source
            .balance
            .checked_sub(settlement.buyer_collateral_debit)
            .ok_or(DirectPhysicalError::Arithmetic)?
        || delegated_amount != expected_residual
        || count > DIRECT_ORDINARY_CUSTODY_EFFECT_CAPACITY_V2
    {
        return Err(DirectPhysicalError::Postcondition);
    }
    Ok(CustodyCompilationV2 {
        effects,
        count: u8::try_from(count).map_err(|_| DirectPhysicalError::Arithmetic)?,
        source_after: source_balance,
        delegated_after: delegated_amount,
        seller_after: seller_balance,
        fee_after: fee_balance,
    })
}

struct CustodyCompilationV2 {
    effects: [Option<DirectCustodyEffectV2>; DIRECT_ORDINARY_CUSTODY_EFFECT_CAPACITY_V2],
    count: u8,
    source_after: u64,
    delegated_after: u64,
    seller_after: u64,
    fee_after: u64,
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

fn custody_request(
    context: DirectOrdinaryPhysicalContextV2,
    buyer_record: DirectRegisteredIntentV2,
    source: DirectExternalDebitV2,
    destination: DirectExternalCollateralV2,
    transfer_index: usize,
    amount: u64,
) -> Result<CustodyRequestV1> {
    let transfer_index_u16 =
        u16::try_from(transfer_index).map_err(|_| DirectPhysicalError::Arithmetic)?;
    let expected_revision = context
        .custody_replay_revision
        .checked_add(u64::try_from(transfer_index).map_err(|_| DirectPhysicalError::Arithmetic)?)
        .ok_or(DirectPhysicalError::Arithmetic)?;
    let request = CustodyRequestV1 {
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
            source_owner: source.owner,
            destination_owner: destination.owner,
            order: context.buyer_record,
            parent_request_digest: context.parent_request_digest,
            order_nonce: buyer_record.intent().nonce,
            generation: context.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: transfer_index_u16,
        },
        source: source.account,
        destination: destination.account,
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: context.mint,
        token_program: context.token_program,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision,
        resulting_revision: expected_revision
            .checked_add(1)
            .ok_or(DirectPhysicalError::Arithmetic)?,
        amount,
        rent_lamports: 0,
    };
    request
        .validate()
        .map_err(|_| DirectPhysicalError::Custody)?;
    Ok(request)
}

fn is_zero(value: [u8; 32]) -> bool {
    value == [0; 32]
}
