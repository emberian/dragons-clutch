//! Claims-owned record Position lifecycle for registered Direct Sell inventory.
//!
//! Registration admits the canonical LBV2 Position whose owner is the exact
//! Direct record, then an affine Claims effect moves the signed maximum fill
//! from maker to record. Fills move claims from record to buyer. Cancellation,
//! expiry, and invalidation return the residual to maker. A terminal zero
//! record Position and its admission state then close to the record's persisted
//! RentCredit. Direct never mirrors the claims vector.

use dclutch_claims_svm::{
    CallerRole as ClaimsCallerRole,
    affine_batch_v2::{AffineBatchPlanV2, AffineBatchReceiptV2, DeltaDirectionV2},
    protocol_position_v2::{
        ProtocolPositionActionV2, ProtocolPositionAdmissionEvidenceV2,
        ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2,
        ProtocolPositionCloseEvidenceV2, ProtocolPositionCloseReceiptV2,
        ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
        ProtocolPositionSeedsV2,
    },
};
use dclutch_direct_codec::successor::{
    DirectRegisteredIntentV2, RegisteredFillCandidateV2, RegisteredIntentCreationV2,
    RegisteredIntentSeedsV2, RegisteredRecordAfterFillV2, RegisteredRecordCloseV2,
};
use dclutch_market_core_codec::{CoreMarketViewV1, Phase};
use solana_program::{hash::hash, pubkey::Pubkey};

use super::physical::{DirectPhysicalError, Result};

/// Exact physical accounts for one Claims-owned protocol Position lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSellPositionAccountsV2 {
    /// Trading-owned Direct record PDA and Claims Position owner coordinate.
    pub record: [u8; 32],
    /// Canonical LBV2 Position PDA under Claims.
    pub position: [u8; 32],
    /// Canonical Claims admission-state PDA.
    pub admission: [u8; 32],
}

/// Exact physical accounts for one ordinary user's Claims Position lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectUserPositionAccountsV2 {
    /// Persisted user identity and Claims Position owner coordinate.
    pub owner: [u8; 32],
    /// Canonical LBV2 Position PDA under Claims.
    pub position: [u8; 32],
    /// Canonical Claims admission-state PDA.
    pub admission: [u8; 32],
}

/// Dust-tolerant prepaid or existing account-lamport observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPositionFundingV2 {
    /// Exact observed Position lamports.
    pub position_lamports: u64,
    /// Exact observed admission-state lamports.
    pub admission_lamports: u64,
    /// Current Position rent minimum persisted as principal.
    pub position_rent_principal: u64,
    /// Current admission-state rent minimum persisted as principal.
    pub admission_rent_principal: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PositionRequestSelectionV2 {
    action: ProtocolPositionActionV2,
    owner_kind: ProtocolPositionOwnerKindV2,
    presence: ProtocolPositionPresenceV2,
    owner: [u8; 32],
    rent_credit: [u8; 32],
    expected_position_revision: u64,
}

/// Authenticated fixed-role and Product facts for Sell escrow effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSellEscrowContextV2 {
    /// Sparse-Core view after exact Market/reference/Registry authentication.
    pub core_market: CoreMarketViewV1,
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// Current Registry-selected Claims program.
    pub claims_program: [u8; 32],
    /// Current pure RentCredit program.
    pub rent_program: [u8; 32],
    /// SHA-256 of the complete canonical parent Trading request.
    pub parent_request_digest: [u8; 32],
    /// Exact finalized linked-LiabilityBasis record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Claims aggregate revision before the selected child effect.
    pub claims_market_revision: u64,
}

impl DirectSellEscrowContextV2 {
    fn validate(self, terminal: bool) -> Result<()> {
        let phase_valid = if terminal {
            matches!(
                self.core_market.phase(),
                Phase::Open | Phase::Terminal | Phase::Retiring
            )
        } else {
            self.core_market.phase() == Phase::Open
        };
        if self.trading_program == [0; 32]
            || self.claims_program == [0; 32]
            || self.rent_program == [0; 32]
            || self.parent_request_digest == [0; 32]
            || self.linked_basis_record_digest == [0; 32]
            || self.core_market.release_set().bindings[1]
                .program
                .to_bytes()
                != self.claims_program
            || self.core_market.release_set().bindings[2]
                .program
                .to_bytes()
                != self.trading_program
            || !phase_valid
        {
            return Err(DirectPhysicalError::Binding);
        }
        Ok(())
    }
}

/// Canonical record-Position admission and reserved-claim amount.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSellRegistrationPlanV2 {
    /// Claims-owned Position admission request.
    pub admission: ProtocolPositionRequestV2,
    /// Exact claims moved maker-to-record by the following affine effect.
    pub reserved_claims: u64,
}

/// Build the exact Claims admission for one accepted registered Sell.
pub fn prepare_sell_registration_v2(
    creation: RegisteredIntentCreationV2,
    accounts: DirectSellPositionAccountsV2,
    funding: DirectPositionFundingV2,
    context: DirectSellEscrowContextV2,
) -> Result<DirectSellRegistrationPlanV2> {
    context.validate(false)?;
    let record = creation.record;
    validate_sell_record(record, context)?;
    validate_record_accounts(record, accounts, context)?;
    if record.reserved_claims() == 0 || record.reserved_collateral() != 0 {
        return Err(DirectPhysicalError::Binding);
    }
    let admission = position_request(
        PositionRequestSelectionV2 {
            action: ProtocolPositionActionV2::Admit,
            owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
            presence: ProtocolPositionPresenceV2::Vacant,
            owner: accounts.record,
            rent_credit: record.rent_owner(),
            expected_position_revision: 0,
        },
        funding,
        context,
    )?;
    Ok(DirectSellRegistrationPlanV2 {
        admission,
        reserved_claims: record.reserved_claims(),
    })
}

/// Verify the immediate Claims admission receipt for a record Position.
pub fn verify_sell_admission_receipt_v2(
    request: ProtocolPositionRequestV2,
    context: DirectSellEscrowContextV2,
    receipt_bytes: &[u8],
) -> Result<()> {
    context.validate(false)?;
    let request_bytes = request
        .to_bytes()
        .map_err(|_| DirectPhysicalError::Claims)?;
    let expected = ProtocolPositionAdmissionV2::new(
        request,
        ProtocolPositionAdmissionEvidenceV2 {
            product_record_digest: context.core_market.product().product_record.to_bytes(),
            semantic_basis_id: context.core_market.product().liability_basis.to_bytes(),
            linked_basis_record_digest: context.linked_basis_record_digest,
            request_digest: hash(&request_bytes).to_bytes(),
            claims_program: context.claims_program,
            trading_program: context.trading_program,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
            outcome_count: context.core_market.product().outcome_count,
        },
    )
    .map_err(|_| DirectPhysicalError::Claims)?;
    let observed = ProtocolPositionAdmissionV2::decode_receipt(receipt_bytes)
        .map_err(|_| DirectPhysicalError::Claims)?;
    if observed != expected {
        return Err(DirectPhysicalError::Postcondition);
    }
    Ok(())
}

/// Build the optional vacant-user admission required before a Sell fill.
///
/// Existing buyer Positions skip this lifecycle effect. A vacant Position is
/// admitted to the exact authenticated beneficiary RentCredit before the
/// record-to-buyer affine transfer; Direct does not create or own the Position.
pub fn prepare_sell_user_admission_v2(
    accounts: DirectUserPositionAccountsV2,
    rent_credit: [u8; 32],
    funding: DirectPositionFundingV2,
    context: DirectSellEscrowContextV2,
) -> Result<ProtocolPositionRequestV2> {
    context.validate(false)?;
    if rent_credit == [0; 32] || accounts.owner == rent_credit {
        return Err(DirectPhysicalError::Binding);
    }
    validate_position_accounts(
        accounts.owner,
        accounts.position,
        accounts.admission,
        context,
    )?;
    position_request(
        PositionRequestSelectionV2 {
            action: ProtocolPositionActionV2::Admit,
            owner_kind: ProtocolPositionOwnerKindV2::User,
            presence: ProtocolPositionPresenceV2::Vacant,
            owner: accounts.owner,
            rent_credit,
            expected_position_revision: 0,
        },
        funding,
        context,
    )
}

/// One exact record-Position affine movement selected by Direct.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectSellAffineActionV2 {
    /// Registration reserves claims from maker into the record Position.
    Register,
    /// A fill releases claims from the record Position to the buyer.
    Fill,
    /// Cancel, expiry, or invalidation refunds the residual to maker.
    Unwind,
}

/// Exact semantic facts for one affine record-Position movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSellAffineExpectationV2 {
    /// Registration, fill, or terminal residual refund.
    pub action: DirectSellAffineActionV2,
    /// Authenticated Sell record before the Direct transition.
    pub record_before: DirectRegisteredIntentV2,
    /// Canonical physical Direct record key.
    pub record: [u8; 32],
    /// Opposite user owner: maker for register/unwind, buyer for fill.
    pub user: [u8; 32],
    /// Exact record Position revision before the affine effect.
    pub record_position_revision: u64,
    /// Exact user Position revision before the affine effect.
    pub user_position_revision: u64,
    /// Exact positive moved claim quantity.
    pub quantity: u64,
}

/// Hostile-decode one descriptor-projected affine packet and require exact Sell semantics.
pub fn validate_sell_affine_plan_v2<'a>(
    expectation: DirectSellAffineExpectationV2,
    context: DirectSellEscrowContextV2,
    plan_bytes: &'a [u8],
) -> Result<AffineBatchPlanV2<'a>> {
    context.validate(expectation.action == DirectSellAffineActionV2::Unwind)?;
    validate_sell_record(expectation.record_before, context)?;
    validate_record_key(
        expectation.record_before,
        expectation.record,
        context.trading_program,
    )?;
    if expectation.user == [0; 32]
        || expectation.user == expectation.record
        || expectation.quantity == 0
        || matches!(expectation.action, DirectSellAffineActionV2::Fill)
            && expectation.user == expectation.record_before.maker()
    {
        return Err(DirectPhysicalError::Binding);
    }
    let expected_quantity = match expectation.action {
        DirectSellAffineActionV2::Register => expectation.record_before.reserved_claims(),
        DirectSellAffineActionV2::Fill => expectation.quantity,
        DirectSellAffineActionV2::Unwind => expectation.record_before.reserved_claims(),
    };
    if expectation.quantity != expected_quantity {
        return Err(DirectPhysicalError::Binding);
    }
    let plan = AffineBatchPlanV2::decode(plan_bytes).map_err(|_| DirectPhysicalError::Claims)?;
    let product = context.core_market.product();
    if plan.caller_role() != ClaimsCallerRole::Trading
        || plan.release_set() != context.core_market.release_set().release_set_id.to_bytes()
        || plan.market() != context.core_market.market().to_bytes()
        || plan.request_id() != context.parent_request_digest
        || plan.product_record_digest() != product.product_record.to_bytes()
        || plan.semantic_basis_id() != product.liability_basis.to_bytes()
        || plan.linked_basis_record_digest() != context.linked_basis_record_digest
        || plan.expected_market_revision() != context.claims_market_revision
        || plan.outcome_count() != product.outcome_count
        || plan.position_count() != 2
        || plan.row_count() != 1
    {
        return Err(DirectPhysicalError::Binding);
    }
    let (source_owner, source_revision, destination_owner, destination_revision) =
        match expectation.action {
            DirectSellAffineActionV2::Register => (
                expectation.user,
                expectation.user_position_revision,
                expectation.record,
                expectation.record_position_revision,
            ),
            DirectSellAffineActionV2::Fill | DirectSellAffineActionV2::Unwind => (
                expectation.record,
                expectation.record_position_revision,
                expectation.user,
                expectation.user_position_revision,
            ),
        };
    let source = plan.position(0).map_err(|_| DirectPhysicalError::Claims)?;
    let destination = plan.position(1).map_err(|_| DirectPhysicalError::Claims)?;
    let row = plan.row(0).map_err(|_| DirectPhysicalError::Claims)?;
    if source.owner() != source_owner
        || source.expected_revision() != source_revision
        || destination.owner() != destination_owner
        || destination.expected_revision() != destination_revision
        || !row.source_present()
        || !row.destination_present()
        || row.outcome() != expectation.record_before.intent().outcome
        || row.source_position_index() != 0
        || row.destination_position_index() != 1
        || !delta(row.aggregate_delta(), DeltaDirectionV2::Neutral, 0)
        || !delta(
            row.source_delta(),
            DeltaDirectionV2::Debit,
            expectation.quantity,
        )
        || !delta(
            row.destination_delta(),
            DeltaDirectionV2::Credit,
            expectation.quantity,
        )
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(plan)
}

/// Verify the exact affine Claims producer receipt.
pub fn verify_sell_affine_receipt_v2(
    expectation: DirectSellAffineExpectationV2,
    context: DirectSellEscrowContextV2,
    plan_bytes: &[u8],
    receipt_bytes: &[u8],
    post_resource_digest: [u8; 32],
) -> Result<()> {
    if post_resource_digest == [0; 32] {
        return Err(DirectPhysicalError::ZeroIdentity);
    }
    let plan = validate_sell_affine_plan_v2(expectation, context, plan_bytes)?;
    let receipt =
        AffineBatchReceiptV2::decode(receipt_bytes).map_err(|_| DirectPhysicalError::Claims)?;
    receipt
        .validate_plan(plan)
        .map_err(|_| DirectPhysicalError::Claims)?;
    if receipt.claims_program() != context.claims_program
        || receipt.post_resource_digest() != post_resource_digest
        || receipt.packet_digest() != hash(plan_bytes).to_bytes()
    {
        return Err(DirectPhysicalError::Postcondition);
    }
    Ok(())
}

/// Close one terminal zero record Position and admission state.
pub fn prepare_sell_close_v2(
    record_before: DirectRegisteredIntentV2,
    close: RegisteredRecordCloseV2,
    accounts: DirectSellPositionAccountsV2,
    admission: ProtocolPositionAdmissionV2,
    post_affine_position_revision: u64,
    current_funding: DirectPositionFundingV2,
    context: DirectSellEscrowContextV2,
) -> Result<ProtocolPositionRequestV2> {
    context.validate(true)?;
    validate_sell_record(record_before, context)?;
    validate_record_accounts(record_before, accounts, context)?;
    validate_record_admission(
        admission,
        accounts.record,
        record_before.rent_owner(),
        context,
    )?;
    if close.closed_nonce != record_before.intent().nonce
        || close.collateral_refund != 0
        || close.rent_owner != record_before.rent_owner()
    {
        return Err(DirectPhysicalError::Binding);
    }
    position_request(
        PositionRequestSelectionV2 {
            action: ProtocolPositionActionV2::Close,
            owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
            presence: ProtocolPositionPresenceV2::Existing,
            owner: accounts.record,
            rent_credit: record_before.rent_owner(),
            expected_position_revision: post_affine_position_revision,
        },
        current_funding,
        context,
    )
}

/// Verify the exact terminal Position/admission reclamation receipt.
pub fn verify_sell_close_receipt_v2(
    request: ProtocolPositionRequestV2,
    context: DirectSellEscrowContextV2,
    rent_credit_before: u64,
    post_resource_digest: [u8; 32],
    admission_state_bytes: &[u8],
    receipt_bytes: &[u8],
) -> Result<()> {
    context.validate(true)?;
    let admission = ProtocolPositionAdmissionV2::decode(admission_state_bytes)
        .map_err(|_| DirectPhysicalError::Claims)?;
    validate_record_admission(
        admission,
        request.position_owner,
        request.rent_credit,
        context,
    )?;
    if post_resource_digest == [0; 32]
        || request.action != ProtocolPositionActionV2::Close
        || request.owner_kind != ProtocolPositionOwnerKindV2::TradingRecord
        || request.presence != ProtocolPositionPresenceV2::Existing
        || request.release_set != context.core_market.release_set().release_set_id.to_bytes()
        || request.market != context.core_market.market().to_bytes()
        || request.rent_program != context.rent_program
        || request.generation != context.core_market.generation()
        || request.expected_market_revision != context.claims_market_revision
        || request.capability_descriptor != [0; 32]
        || request.capability_outcome != 0
    {
        return Err(DirectPhysicalError::Binding);
    }
    let request_bytes = request
        .to_bytes()
        .map_err(|_| DirectPhysicalError::Claims)?;
    let total_credit = request
        .observed_position_lamports
        .checked_add(request.observed_admission_lamports)
        .ok_or(DirectPhysicalError::Arithmetic)?;
    let expected = ProtocolPositionCloseReceiptV2::new(
        request,
        ProtocolPositionCloseEvidenceV2 {
            request_digest: hash(&request_bytes).to_bytes(),
            admission_digest: hash(admission_state_bytes).to_bytes(),
            claims_program: context.claims_program,
            post_resource_digest,
            rent_credit_before,
            rent_credit_after: rent_credit_before
                .checked_add(total_credit)
                .ok_or(DirectPhysicalError::Arithmetic)?,
        },
    )
    .map_err(|_| DirectPhysicalError::Claims)?;
    let observed = ProtocolPositionCloseReceiptV2::decode(receipt_bytes)
        .map_err(|_| DirectPhysicalError::Claims)?;
    if observed != expected {
        return Err(DirectPhysicalError::Postcondition);
    }
    Ok(())
}

/// Require a full/partial fill candidate to debit precisely the record Position.
pub fn sell_fill_expectation_v2(
    record_before: DirectRegisteredIntentV2,
    record: [u8; 32],
    candidate: RegisteredFillCandidateV2,
    buyer: [u8; 32],
    record_position_revision: u64,
    buyer_position_revision: u64,
) -> Result<DirectSellAffineExpectationV2> {
    let quantity = candidate.effects.claim_custody_debit;
    let nonce = match candidate.record {
        RegisteredRecordAfterFillV2::Live(after) => {
            if after.maker() != record_before.maker() || after.intent() != record_before.intent() {
                return Err(DirectPhysicalError::Binding);
            }
            after.intent().nonce
        }
        RegisteredRecordAfterFillV2::Closed(close) => close.closed_nonce,
    };
    if nonce != record_before.intent().nonce
        || quantity == 0
        || candidate.effects.claim_position_credit != 0
        || candidate.maker_root.maker() != record_before.maker()
        || candidate.maker_root.market() != record_before.intent().market
        || candidate.maker_root.generation() != record_before.intent().generation
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(DirectSellAffineExpectationV2 {
        action: DirectSellAffineActionV2::Fill,
        record_before,
        record,
        user: buyer,
        record_position_revision,
        user_position_revision: buyer_position_revision,
        quantity,
    })
}

fn position_request(
    selection: PositionRequestSelectionV2,
    funding: DirectPositionFundingV2,
    context: DirectSellEscrowContextV2,
) -> Result<ProtocolPositionRequestV2> {
    ProtocolPositionRequestV2 {
        action: selection.action,
        owner_kind: selection.owner_kind,
        presence: selection.presence,
        release_set: context.core_market.release_set().release_set_id.to_bytes(),
        market: context.core_market.market().to_bytes(),
        position_owner: selection.owner,
        parent_request_digest: context.parent_request_digest,
        rent_credit: selection.rent_credit,
        rent_program: context.rent_program,
        generation: context.core_market.generation(),
        expected_market_revision: context.claims_market_revision,
        expected_position_revision: selection.expected_position_revision,
        observed_position_lamports: funding.position_lamports,
        observed_admission_lamports: funding.admission_lamports,
        position_rent_principal: funding.position_rent_principal,
        admission_rent_principal: funding.admission_rent_principal,
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
    .new()
    .map_err(|_| DirectPhysicalError::Claims)
}

fn validate_sell_record(
    record: DirectRegisteredIntentV2,
    context: DirectSellEscrowContextV2,
) -> Result<()> {
    if record.intent().side != 0
        || record.intent().lifecycle != 2
        || record.intent().market != context.core_market.market().to_bytes()
        || record.intent().generation != context.core_market.generation()
        || record.intent().outcome >= context.core_market.product().outcome_count
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn validate_record_accounts(
    record: DirectRegisteredIntentV2,
    accounts: DirectSellPositionAccountsV2,
    context: DirectSellEscrowContextV2,
) -> Result<()> {
    validate_record_key(record, accounts.record, context.trading_program)?;
    validate_position_accounts(
        accounts.record,
        accounts.position,
        accounts.admission,
        context,
    )
}

fn validate_position_accounts(
    owner: [u8; 32],
    position: [u8; 32],
    admission: [u8; 32],
    context: DirectSellEscrowContextV2,
) -> Result<()> {
    if owner == [0; 32]
        || position == [0; 32]
        || admission == [0; 32]
        || position == admission
        || position == owner
        || admission == owner
    {
        return Err(DirectPhysicalError::Binding);
    }
    let aggregate = context.core_market.claims_aggregate().to_bytes();
    let position_seeds =
        ProtocolPositionSeedsV2::new(aggregate, owner).map_err(|_| DirectPhysicalError::Claims)?;
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(aggregate, owner)
        .map_err(|_| DirectPhysicalError::Claims)?;
    if derive(context.claims_program, &position_seeds.as_slices()).0 != position
        || derive(context.claims_program, &admission_seeds.as_slices()).0 != admission
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn validate_record_admission(
    admission: ProtocolPositionAdmissionV2,
    record: [u8; 32],
    rent_credit: [u8; 32],
    context: DirectSellEscrowContextV2,
) -> Result<()> {
    if admission.owner_kind() != ProtocolPositionOwnerKindV2::TradingRecord
        || admission.position_owner() != record
        || admission.market() != context.core_market.market().to_bytes()
        || admission.release_set() != context.core_market.release_set().release_set_id.to_bytes()
        || admission.generation() != context.core_market.generation()
        || admission.rent_credit() != rent_credit
        || admission.rent_program() != context.rent_program
        || admission.claims_program() != context.claims_program
        || admission.trading_program() != context.trading_program
        || admission.product_record_digest()
            != context.core_market.product().product_record.to_bytes()
        || admission.semantic_basis_id() != context.core_market.product().liability_basis.to_bytes()
        || admission.linked_basis_record_digest() != context.linked_basis_record_digest
        || admission.outcome_count() != context.core_market.product().outcome_count
        || admission.capability_descriptor() != [0; 32]
        || admission.capability_outcome() != 0
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn validate_record_key(
    record: DirectRegisteredIntentV2,
    record_key: [u8; 32],
    trading_program: [u8; 32],
) -> Result<()> {
    let seeds = RegisteredIntentSeedsV2::from_record(record);
    let (expected, bump) = derive(trading_program, &seeds.as_slices());
    if expected != record_key || bump != record.bump() {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn delta(
    value: dclutch_claims_svm::affine_batch_v2::SignedMagnitudeV2,
    direction: DeltaDirectionV2,
    magnitude: u64,
) -> bool {
    value.direction() == direction && value.magnitude() == magnitude
}

fn derive(program: [u8; 32], seeds: &[&[u8]]) -> ([u8; 32], u8) {
    let (address, bump) = Pubkey::find_program_address(seeds, &Pubkey::new_from_array(program));
    (address.to_bytes(), bump)
}
