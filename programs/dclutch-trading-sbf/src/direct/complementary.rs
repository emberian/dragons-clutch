//! Runtime-width Direct complementary Custody projection.
//!
//! Claims remains the sole owner of the affine N-position liability mutation.
//! This module projects only each already-checked participant's two canonical
//! Custody routes: principal/net first, then fee. It has no width specialization
//! and no caller-authored effect authority.

use dclutch_claims_svm::{
    CallerRole as ClaimsCallerRole,
    affine_batch_v2::{
        AffineBatchPlanV2, AffineBatchPositionV2, AffineBatchReceiptV2, DeltaDirectionV2,
    },
};
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyReplayV1, CustodyRequestV1, OperationV1,
};
use dclutch_direct_codec::successor::{
    ComplementaryActionV2, ComplementarySettlementV2, DirectExecutionConfigV1,
    DirectRegisteredIntentV2, RegisteredFillCandidateV2, RegisteredIntentSeedsV2,
    RegisteredRecordAfterFillV2,
};
use dclutch_market_core_codec::{CoreMarketViewV1, Phase};
use solana_program::{
    hash::{hash, hashv},
    pubkey::Pubkey,
};

use super::{
    buy_escrow::{
        DirectBuyEscrowAccountsV2, DirectBuyEscrowContextV2, OperationShapeV2,
        request as buy_escrow_request, validate_accounts as validate_buy_escrow_accounts,
        validate_replay as validate_buy_escrow_replay,
    },
    physical::{DirectExternalCollateralV2, DirectPhysicalError, Result},
};

/// Canonical route order for every complementary participant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectComplementaryCustodyRouteV2 {
    /// Split gross into Hoard principal, or merge net out to the seller.
    PrincipalOrNet,
    /// Charge the participant's cumulative-difference fee.
    Fee,
    /// Return a terminal price-improvement residual to the signed Buy source.
    Residual,
    /// Close the zero-balance record-keyed Buy Vault.
    CloseBuyVault,
    /// Close the quiescent record-keyed Buy replay cursor.
    CloseBuyReplay,
}

/// Exact fixed-role facts common to every complementary Custody effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryPhysicalContextV2 {
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// Sparse-Core view after exact Market, reference-record, and Registry authentication.
    pub core_market: CoreMarketViewV1,
    /// Canonical Custody transfer-authority PDA.
    pub custody_authority: [u8; 32],
    /// SHA-256 of the complete canonical parent Trading request.
    pub parent_request_digest: [u8; 32],
    /// Canonical Custody-owned HoardPrincipal token account.
    pub hoard_token_account: [u8; 32],
    /// Authenticated HoardPrincipal token balance before the complete action.
    pub hoard_balance: u64,
    /// Immutable config-recipient external token account.
    pub fee_destination: DirectExternalCollateralV2,
}

impl DirectComplementaryPhysicalContextV2 {
    fn validate(self, config: DirectExecutionConfigV1) -> Result<()> {
        let release_set = self.core_market.release_set();
        let realm = self.core_market.realm();
        for identity in [
            self.trading_program,
            self.core_market.market().to_bytes(),
            self.core_market.claims_aggregate().to_bytes(),
            release_set.release_set_id.to_bytes(),
            realm.realm_id.to_bytes(),
            realm.collateral_mint.to_bytes(),
            realm.token_program.to_bytes(),
            self.custody_authority,
            self.parent_request_digest,
            self.hoard_token_account,
            self.fee_destination.account,
            self.fee_destination.owner,
        ] {
            if identity == [0; 32] {
                return Err(DirectPhysicalError::ZeroIdentity);
            }
        }
        if self.core_market.phase() != Phase::Open
            || release_set.bindings[2].program.to_bytes() != self.trading_program
            || self.fee_destination.owner != config.fee_recipient()
            || self.fee_destination.account == self.hoard_token_account
            || self.core_market.claims_aggregate().to_bytes() == self.hoard_token_account
        {
            return Err(DirectPhysicalError::Binding);
        }
        Ok(())
    }

    const fn market(self) -> [u8; 32] {
        self.core_market.market().to_bytes()
    }

    const fn claims_aggregate(self) -> [u8; 32] {
        self.core_market.claims_aggregate().to_bytes()
    }

    const fn release_set(self) -> [u8; 32] {
        self.core_market.release_set().release_set_id.to_bytes()
    }

    const fn realm(self) -> [u8; 32] {
        self.core_market.realm().realm_id.to_bytes()
    }

    const fn mint(self) -> [u8; 32] {
        self.core_market.realm().collateral_mint.to_bytes()
    }

    const fn token_program(self) -> [u8; 32] {
        self.core_market.realm().token_program.to_bytes()
    }

    const fn generation(self) -> u64 {
        self.core_market.generation()
    }
}

/// Side-specific authenticated participant collateral endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectComplementaryCollateralV2<'a> {
    /// Registered Buy reserve held in exact record-keyed Custody.
    BuyEscrow(&'a DirectComplementaryBuyEscrowV2),
    /// Registered Sell external destination.
    SellDestination(DirectExternalCollateralV2),
}

/// Authenticated record-keyed Custody state for one registered Buy participant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryBuyEscrowV2 {
    /// Canonical record/replay/Vault/authority coordinates.
    pub accounts: DirectBuyEscrowAccountsV2,
    /// Current exact Custody replay state.
    pub replay: CustodyReplayV1,
    /// Current exact `TradingPrincipal` Vault balance.
    pub vault_balance: u64,
    /// Signed Buy source receiving any terminal reserve residual.
    pub refund_destination: DirectExternalCollateralV2,
    /// Exact Vault lamports recovered on terminal close.
    pub vault_rent_lamports: u64,
    /// Exact replay lamports recovered on terminal close.
    pub replay_rent_lamports: u64,
}

/// Authenticated physical coordinates for one canonical outcome participant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryParticipantV2<'a> {
    /// Exact Direct maker-root account and Custody replay context.
    pub maker_root: [u8; 32],
    /// Exact live registered-record account.
    pub record: [u8; 32],
    /// Side-specific collateral account observation.
    pub collateral: DirectComplementaryCollateralV2<'a>,
    /// Per-maker Custody replay revision before this participant's first effect.
    pub custody_replay_revision: u64,
}

/// Complete semantic input for one route and one canonical Product outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryProjectionInputV2<'a> {
    /// Split or merge.
    pub action: ComplementaryActionV2,
    /// Principal/net or fee route.
    pub route: DirectComplementaryCustodyRouteV2,
    /// Product-owned canonical outcome coordinate.
    pub participant_index: u32,
    /// Authenticated pre-fill record retained even when the candidate closes.
    pub record_before: DirectRegisteredIntentV2,
    /// Sole checked Direct participant candidate.
    pub candidate: RegisteredFillCandidateV2,
    /// Authenticated account/token observations.
    pub participant: DirectComplementaryParticipantV2<'a>,
    /// Immutable selected Direct economics.
    pub config: DirectExecutionConfigV1,
    /// Fixed-role Market/Realm/Custody facts.
    pub context: DirectComplementaryPhysicalContextV2,
}

/// One positive, descriptor-routed Custody request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryCustodyEffectV2 {
    /// Canonical Custody request.
    pub request: CustodyRequestV1,
    /// Record-keyed Buy Vault balance after this route, absent for Sell routes.
    pub buy_vault_after: Option<u64>,
}

/// Aggregate token poststate checked before the first complementary CPI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryCustodyAggregateV2 {
    /// HoardPrincipal balance after all principal/net routes.
    pub hoard_after: u64,
    /// Venue-fee destination balance after every fee route.
    pub fee_after: u64,
}

/// Authenticated fixed facts for one runtime-width complementary Claims batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryClaimsContextV2 {
    /// Sparse-Core view after exact Market, Product, Realm, and Registry authentication.
    pub core_market: CoreMarketViewV1,
    /// Current Registry-selected Claims program.
    pub claims_program: [u8; 32],
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// SHA-256 of the complete canonical parent Trading request.
    pub parent_request_digest: [u8; 32],
    /// Finalized linked-LiabilityBasis record digest authenticated by Claims.
    pub linked_basis_record_digest: [u8; 32],
    /// Claims aggregate revision before the batch.
    pub claims_market_revision: u64,
    /// Positive complete-set quantity shared by every outcome row.
    pub fill: u64,
}

impl DirectComplementaryClaimsContextV2 {
    fn validate(self) -> Result<()> {
        let release_set = self.core_market.release_set();
        if self.claims_program == [0; 32]
            || self.trading_program == [0; 32]
            || self.parent_request_digest == [0; 32]
            || self.linked_basis_record_digest == [0; 32]
            || self.fill == 0
            || self.core_market.phase() != Phase::Open
            || release_set.bindings[1].program.to_bytes() != self.claims_program
            || release_set.bindings[2].program.to_bytes() != self.trading_program
        {
            return Err(DirectPhysicalError::Binding);
        }
        Ok(())
    }
}

/// One authenticated Direct participant joined to an existing Claims Position.
///
/// Position creation, rent ownership, and terminal close are intentionally not
/// represented here. Those facts belong to the separate Claims-owned Position
/// lifecycle effect and must have completed before this mutation-only batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryClaimsParticipantV2 {
    /// Authenticated live Direct record before settlement.
    pub record_before: DirectRegisteredIntentV2,
    /// Sole checked Direct candidate for this outcome.
    pub candidate: RegisteredFillCandidateV2,
    /// Exact Trading-owned registered-record PDA.
    pub record: [u8; 32],
    /// Authenticated existing Claims Position revision.
    pub expected_position_revision: u64,
}

/// Hostile-decode and validate the batch-wide Claims facts and canonical rows.
///
/// The EffectProgram constructs the request buffer. Direct validates it rather
/// than owning another Claims packet DTO. The unique Position table is ordered
/// by first appearance in canonical outcome order; repeated makers reuse their
/// first table index.
pub fn validate_complementary_claims_plan_v2<'a>(
    action: ComplementaryActionV2,
    context: DirectComplementaryClaimsContextV2,
    plan_bytes: &'a [u8],
) -> Result<AffineBatchPlanV2<'a>> {
    context.validate()?;
    let plan = AffineBatchPlanV2::decode(plan_bytes).map_err(|_| DirectPhysicalError::Claims)?;
    let product = context.core_market.product();
    let release_set = context.core_market.release_set();
    let outcome_count = product.outcome_count;
    if plan.caller_role() != ClaimsCallerRole::Trading
        || plan.release_set() != release_set.release_set_id.to_bytes()
        || plan.market() != context.core_market.market().to_bytes()
        || plan.request_id() != context.parent_request_digest
        || plan.product_record_digest() != product.product_record.to_bytes()
        || plan.semantic_basis_id() != product.liability_basis.to_bytes()
        || plan.linked_basis_record_digest() != context.linked_basis_record_digest
        || plan.expected_market_revision() != context.claims_market_revision
        || plan.outcome_count() != outcome_count
        || plan.row_count() != outcome_count
        || plan.position_count() == 0
        || plan.position_count() > outcome_count
    {
        return Err(DirectPhysicalError::Binding);
    }

    let mut next_position = 0_u32;
    for index in 0..outcome_count {
        let row = plan.row(index).map_err(|_| DirectPhysicalError::Claims)?;
        if row.outcome() != index {
            return Err(DirectPhysicalError::Binding);
        }
        let position_index = match action {
            ComplementaryActionV2::Split => {
                if row.source_present()
                    || !row.destination_present()
                    || !is_delta(
                        row.aggregate_delta(),
                        DeltaDirectionV2::Credit,
                        context.fill,
                    )
                    || !is_delta(row.source_delta(), DeltaDirectionV2::Neutral, 0)
                    || !is_delta(
                        row.destination_delta(),
                        DeltaDirectionV2::Credit,
                        context.fill,
                    )
                {
                    return Err(DirectPhysicalError::Binding);
                }
                row.destination_position_index()
            }
            ComplementaryActionV2::Merge => {
                if !row.source_present()
                    || row.destination_present()
                    || !is_delta(row.aggregate_delta(), DeltaDirectionV2::Debit, context.fill)
                    || !is_delta(row.source_delta(), DeltaDirectionV2::Debit, context.fill)
                    || !is_delta(row.destination_delta(), DeltaDirectionV2::Neutral, 0)
                {
                    return Err(DirectPhysicalError::Binding);
                }
                row.source_position_index()
            }
        };
        if position_index > next_position {
            return Err(DirectPhysicalError::Binding);
        }
        if position_index == next_position {
            next_position = next_position
                .checked_add(1)
                .ok_or(DirectPhysicalError::Arithmetic)?;
        }
    }
    if next_position != plan.position_count() {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(plan)
}

/// Validate one canonical outcome row against its checked Direct participant.
pub fn validate_complementary_claims_item_v2(
    action: ComplementaryActionV2,
    context: DirectComplementaryClaimsContextV2,
    plan: AffineBatchPlanV2<'_>,
    outcome: u32,
    participant: DirectComplementaryClaimsParticipantV2,
) -> Result<()> {
    context.validate()?;
    if outcome >= plan.outcome_count()
        || plan.market() != context.core_market.market().to_bytes()
        || participant.record_before.intent().market != plan.market()
        || participant.record_before.intent().generation != context.core_market.generation()
        || participant.record_before.intent().outcome != outcome
        || participant.record == [0; 32]
        || participant.candidate.maker_root.market() != plan.market()
        || participant.candidate.maker_root.generation() != context.core_market.generation()
        || participant.candidate.maker_root.maker() != participant.record_before.maker()
    {
        return Err(DirectPhysicalError::Binding);
    }
    validate_candidate_record(participant.record_before, participant.candidate)?;
    let seeds = RegisteredIntentSeedsV2::from_record(participant.record_before);
    let (expected_record, bump) = Pubkey::find_program_address(
        &seeds.as_slices(),
        &Pubkey::new_from_array(context.trading_program),
    );
    if expected_record.to_bytes() != participant.record || bump != participant.record_before.bump()
    {
        return Err(DirectPhysicalError::Binding);
    }
    validate_candidate_claims_effect(action, context.fill, participant)?;

    let row = plan.row(outcome).map_err(|_| DirectPhysicalError::Claims)?;
    let position_index = match action {
        ComplementaryActionV2::Split => row.destination_position_index(),
        ComplementaryActionV2::Merge => row.source_position_index(),
    };
    let position: AffineBatchPositionV2 = plan
        .position(position_index)
        .map_err(|_| DirectPhysicalError::Claims)?;
    let expected_owner = match action {
        ComplementaryActionV2::Split => participant.record_before.maker(),
        ComplementaryActionV2::Merge => participant.record,
    };
    if position.owner() != expected_owner
        || position.expected_revision() != participant.expected_position_revision
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

/// Verify the exact Claims producer receipt after every row has been validated.
pub fn verify_direct_complementary_claims_receipt_v2(
    action: ComplementaryActionV2,
    context: DirectComplementaryClaimsContextV2,
    plan_bytes: &[u8],
    receipt_bytes: &[u8],
    expected_post_resource_digest: [u8; 32],
) -> Result<()> {
    if expected_post_resource_digest == [0; 32] {
        return Err(DirectPhysicalError::ZeroIdentity);
    }
    let plan = validate_complementary_claims_plan_v2(action, context, plan_bytes)?;
    let receipt =
        AffineBatchReceiptV2::decode(receipt_bytes).map_err(|_| DirectPhysicalError::Claims)?;
    receipt
        .validate_plan(plan)
        .map_err(|_| DirectPhysicalError::Claims)?;
    let (positions, rows) = plan.table_bytes();
    if receipt.packet_digest() != hash(plan_bytes).to_bytes()
        || receipt.table_digest() != hashv(&[positions, rows]).to_bytes()
        || receipt.claims_program() != context.claims_program
        || receipt.post_resource_digest() != expected_post_resource_digest
    {
        return Err(DirectPhysicalError::Postcondition);
    }
    Ok(())
}

/// Preflight the complete shared Hoard and fee-destination arithmetic.
///
/// This must accept before the common outer invokes the first affine route, so
/// an insufficient terminal merge cannot depend on transaction rollback as its
/// primary validation mechanism.
pub fn validate_complementary_custody_aggregate_v2(
    action: ComplementaryActionV2,
    settlement: ComplementarySettlementV2,
    config: DirectExecutionConfigV1,
    context: DirectComplementaryPhysicalContextV2,
) -> Result<DirectComplementaryCustodyAggregateV2> {
    context.validate(config)?;
    let hoard_after = match action {
        ComplementaryActionV2::Split => context
            .hoard_balance
            .checked_add(settlement.market_vault_transfer),
        ComplementaryActionV2::Merge => context
            .hoard_balance
            .checked_sub(settlement.market_vault_transfer),
    }
    .ok_or(DirectPhysicalError::Arithmetic)?;
    let fee_after = context
        .fee_destination
        .balance
        .checked_add(settlement.total_fee_transfer)
        .ok_or(DirectPhysicalError::Arithmetic)?;
    Ok(DirectComplementaryCustodyAggregateV2 {
        hoard_after,
        fee_after,
    })
}

/// Project one route/item of a checked complementary settlement.
///
/// The common EffectProgram V3 invokes `PrincipalOrNet` for every canonical
/// item, then `Fee` for every item. Zero amounts produce no invocation. Each
/// maker's replay indices remain consecutive even though the two affine routes
/// are globally separated.
pub fn project_complementary_custody_effect_v2(
    input: DirectComplementaryProjectionInputV2<'_>,
) -> Result<Option<DirectComplementaryCustodyEffectV2>> {
    input.context.validate(input.config)?;
    authenticate_coordinate(input)?;
    if let DirectComplementaryCollateralV2::BuyEscrow(escrow) = input.participant.collateral {
        return project_buy_escrow_effect(input, *escrow);
    }
    let shape = effect_shape(input)?;
    if shape.amount == 0 {
        return Ok(None);
    }
    let principal_positive = input.candidate.effects.net_collateral_credit != 0;
    let transfer_index = match input.route {
        DirectComplementaryCustodyRouteV2::PrincipalOrNet => 0,
        DirectComplementaryCustodyRouteV2::Fee if principal_positive => 1,
        DirectComplementaryCustodyRouteV2::Fee => 0,
        DirectComplementaryCustodyRouteV2::Residual
        | DirectComplementaryCustodyRouteV2::CloseBuyVault
        | DirectComplementaryCustodyRouteV2::CloseBuyReplay => {
            return Err(DirectPhysicalError::Binding);
        }
    };
    let expected_revision = input
        .participant
        .custody_replay_revision
        .checked_add(u64::from(transfer_index))
        .ok_or(DirectPhysicalError::Arithmetic)?;
    let (source_owner, destination_owner, source_vault_context, destination_vault_context) =
        owner_and_vault_shape(input);
    let request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Trading,
        source_compartment: shape.source_compartment,
        destination_compartment: shape.destination_compartment,
        release_set: input.context.release_set(),
        market: input.context.market(),
        realm: input.context.realm(),
        context: input.participant.maker_root,
        caller_program: input.context.trading_program,
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner,
            destination_owner,
            order: input.participant.record,
            parent_request_digest: input.context.parent_request_digest,
            order_nonce: input.record_before.intent().nonce,
            generation: input.context.generation(),
            page_index: 0,
            execution_index: input.participant_index,
            transfer_index,
        },
        source: shape.source,
        destination: shape.destination,
        source_vault_context,
        destination_vault_context,
        mint: input.context.mint(),
        token_program: input.context.token_program(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision,
        resulting_revision: expected_revision
            .checked_add(1)
            .ok_or(DirectPhysicalError::Arithmetic)?,
        amount: shape.amount,
        rent_lamports: 0,
    };
    request
        .validate()
        .map_err(|_| DirectPhysicalError::Custody)?;
    Ok(Some(DirectComplementaryCustodyEffectV2 {
        request,
        buy_vault_after: None,
    }))
}

fn project_buy_escrow_effect(
    input: DirectComplementaryProjectionInputV2<'_>,
    escrow: DirectComplementaryBuyEscrowV2,
) -> Result<Option<DirectComplementaryCustodyEffectV2>> {
    if input.action != ComplementaryActionV2::Split
        || input.record_before.intent().side != 1
        || input.participant.record != escrow.accounts.record
        || input.participant.custody_replay_revision != escrow.replay.next_revision
        || escrow.accounts.custody_authority != input.context.custody_authority
        || escrow.vault_balance != input.record_before.reserved_collateral()
        || escrow.refund_destination.account != input.record_before.intent().collateral_account
        || escrow.refund_destination.owner != input.record_before.maker()
    {
        return Err(DirectPhysicalError::Binding);
    }
    let context = DirectBuyEscrowContextV2 {
        core_market: input.context.core_market,
        trading_program: input.context.trading_program,
        parent_request_digest: input.context.parent_request_digest,
    };
    validate_buy_escrow_accounts(context, input.record_before, escrow.accounts)?;
    validate_buy_escrow_replay(
        context,
        input.record_before,
        escrow.accounts,
        escrow.replay,
        1,
    )?;

    let principal = input.candidate.effects.gross_collateral_debit;
    let fee = input.candidate.effects.fee_transfer;
    let (residual, closed) = match input.candidate.record {
        RegisteredRecordAfterFillV2::Live(record) => (record.reserved_collateral(), false),
        RegisteredRecordAfterFillV2::Closed(close) => (close.collateral_refund, true),
    };
    if principal
        .checked_add(fee)
        .and_then(|spent| spent.checked_add(residual))
        != Some(escrow.vault_balance)
    {
        return Err(DirectPhysicalError::Postcondition);
    }
    if closed && (escrow.vault_rent_lamports == 0 || escrow.replay_rent_lamports == 0) {
        return Err(DirectPhysicalError::Binding);
    }

    let principal_count = u64::from(principal != 0);
    let fee_count = u64::from(fee != 0);
    let residual_count = u64::from(closed && residual != 0);
    let (amount, offset, shape, vault_after) = match input.route {
        DirectComplementaryCustodyRouteV2::PrincipalOrNet => (
            principal,
            0,
            OperationShapeV2::Withdraw {
                destination: input.context.hoard_token_account,
                destination_owner: [0; 32],
                destination_compartment: CompartmentV1::HoardPrincipal,
                destination_vault_context: input.context.claims_aggregate(),
                amount: principal,
            },
            escrow
                .vault_balance
                .checked_sub(principal)
                .ok_or(DirectPhysicalError::Arithmetic)?,
        ),
        DirectComplementaryCustodyRouteV2::Fee => (
            fee,
            principal_count,
            OperationShapeV2::Withdraw {
                destination: input.context.fee_destination.account,
                destination_owner: input.context.fee_destination.owner,
                destination_compartment: CompartmentV1::External,
                destination_vault_context: [0; 32],
                amount: fee,
            },
            escrow
                .vault_balance
                .checked_sub(principal)
                .and_then(|value| value.checked_sub(fee))
                .ok_or(DirectPhysicalError::Arithmetic)?,
        ),
        DirectComplementaryCustodyRouteV2::Residual if closed => (
            residual,
            principal_count
                .checked_add(fee_count)
                .ok_or(DirectPhysicalError::Arithmetic)?,
            OperationShapeV2::Withdraw {
                destination: escrow.refund_destination.account,
                destination_owner: escrow.refund_destination.owner,
                destination_compartment: CompartmentV1::External,
                destination_vault_context: [0; 32],
                amount: residual,
            },
            0,
        ),
        DirectComplementaryCustodyRouteV2::CloseBuyVault if closed => (
            1,
            principal_count
                .checked_add(fee_count)
                .and_then(|value| value.checked_add(residual_count))
                .ok_or(DirectPhysicalError::Arithmetic)?,
            OperationShapeV2::CloseVault {
                rent_refund: input.record_before.rent_owner(),
                rent_lamports: escrow.vault_rent_lamports,
            },
            0,
        ),
        DirectComplementaryCustodyRouteV2::CloseBuyReplay if closed => (
            1,
            principal_count
                .checked_add(fee_count)
                .and_then(|value| value.checked_add(residual_count))
                .and_then(|value| value.checked_add(1))
                .ok_or(DirectPhysicalError::Arithmetic)?,
            OperationShapeV2::CloseReplay {
                rent_refund: input.record_before.rent_owner(),
                rent_lamports: escrow.replay_rent_lamports,
            },
            0,
        ),
        DirectComplementaryCustodyRouteV2::Residual
        | DirectComplementaryCustodyRouteV2::CloseBuyVault
        | DirectComplementaryCustodyRouteV2::CloseBuyReplay => {
            return Ok(None);
        }
    };
    if amount == 0 {
        return Ok(None);
    }
    let expected_revision = escrow
        .replay
        .next_revision
        .checked_add(offset)
        .ok_or(DirectPhysicalError::Arithmetic)?;
    let transfer_index = u16::try_from(offset).map_err(|_| DirectPhysicalError::Arithmetic)?;
    let request = buy_escrow_request(
        context,
        input.record_before,
        escrow.accounts,
        shape,
        expected_revision,
        transfer_index,
    )?;
    Ok(Some(DirectComplementaryCustodyEffectV2 {
        request,
        buy_vault_after: Some(vault_after),
    }))
}

/// Require a descriptor-projected Custody request to equal the Direct candidate.
///
/// This makes the Claims aggregate, rather than the logical Market, the sole
/// accepted HoardPrincipal vault context.
pub fn validate_complementary_custody_request_v2(
    effect: DirectComplementaryCustodyEffectV2,
    observed: CustodyRequestV1,
) -> Result<()> {
    if effect.request != observed {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn authenticate_coordinate(input: DirectComplementaryProjectionInputV2<'_>) -> Result<()> {
    let intent = input.record_before.intent();
    let candidate_nonce = match input.candidate.record {
        RegisteredRecordAfterFillV2::Live(record) => {
            if record.intent() != intent || record.maker() != input.record_before.maker() {
                return Err(DirectPhysicalError::Binding);
            }
            record.intent().nonce
        }
        RegisteredRecordAfterFillV2::Closed(close) => close.closed_nonce,
    };
    if input.participant.maker_root == [0; 32]
        || input.participant.record == [0; 32]
        || input.candidate.maker_root.maker() != input.record_before.maker()
        || input.candidate.maker_root.market() != input.context.market()
        || input.candidate.maker_root.generation() != input.context.generation()
        || intent.market != input.context.market()
        || intent.generation != input.context.generation()
        || intent.outcome != input.participant_index
        || candidate_nonce != intent.nonce
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn validate_candidate_record(
    before: DirectRegisteredIntentV2,
    candidate: RegisteredFillCandidateV2,
) -> Result<()> {
    let candidate_nonce = match candidate.record {
        RegisteredRecordAfterFillV2::Live(after) => {
            if after.intent() != before.intent() || after.maker() != before.maker() {
                return Err(DirectPhysicalError::Binding);
            }
            after.intent().nonce
        }
        RegisteredRecordAfterFillV2::Closed(close) => close.closed_nonce,
    };
    if candidate_nonce != before.intent().nonce {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn validate_candidate_claims_effect(
    action: ComplementaryActionV2,
    fill: u64,
    participant: DirectComplementaryClaimsParticipantV2,
) -> Result<()> {
    let effects = participant.candidate.effects;
    match action {
        ComplementaryActionV2::Split
            if participant.record_before.intent().side == 1
                && effects.claim_custody_debit == 0
                && effects.claim_position_credit == fill =>
        {
            Ok(())
        }
        ComplementaryActionV2::Merge
            if participant.record_before.intent().side == 0
                && effects.claim_custody_debit == fill
                && effects.claim_position_credit == 0 =>
        {
            Ok(())
        }
        ComplementaryActionV2::Split | ComplementaryActionV2::Merge => {
            Err(DirectPhysicalError::Binding)
        }
    }
}

fn is_delta(
    delta: dclutch_claims_svm::affine_batch_v2::SignedMagnitudeV2,
    direction: DeltaDirectionV2,
    magnitude: u64,
) -> bool {
    delta.direction() == direction && delta.magnitude() == magnitude
}

struct CustodyShapeV2 {
    source: [u8; 32],
    destination: [u8; 32],
    source_compartment: CompartmentV1,
    destination_compartment: CompartmentV1,
    amount: u64,
}

fn effect_shape(input: DirectComplementaryProjectionInputV2<'_>) -> Result<CustodyShapeV2> {
    match (input.action, input.route, input.participant.collateral) {
        (
            ComplementaryActionV2::Merge,
            DirectComplementaryCustodyRouteV2::PrincipalOrNet,
            DirectComplementaryCollateralV2::SellDestination(destination),
        ) => {
            authenticate_sell(input, destination)?;
            Ok(CustodyShapeV2 {
                source: input.context.hoard_token_account,
                destination: destination.account,
                source_compartment: CompartmentV1::HoardPrincipal,
                destination_compartment: CompartmentV1::External,
                amount: input.candidate.effects.net_collateral_credit,
            })
        }
        (
            ComplementaryActionV2::Merge,
            DirectComplementaryCustodyRouteV2::Fee,
            DirectComplementaryCollateralV2::SellDestination(destination),
        ) => {
            authenticate_sell(input, destination)?;
            Ok(CustodyShapeV2 {
                source: input.context.hoard_token_account,
                destination: input.context.fee_destination.account,
                source_compartment: CompartmentV1::HoardPrincipal,
                destination_compartment: CompartmentV1::External,
                amount: input.candidate.effects.fee_transfer,
            })
        }
        _ => Err(DirectPhysicalError::Binding),
    }
}

fn owner_and_vault_shape(
    input: DirectComplementaryProjectionInputV2<'_>,
) -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
    match input.participant.collateral {
        DirectComplementaryCollateralV2::BuyEscrow(_) => ([0; 32], [0; 32], [0; 32], [0; 32]),
        DirectComplementaryCollateralV2::SellDestination(destination) => (
            [0; 32],
            if input.route == DirectComplementaryCustodyRouteV2::Fee {
                input.context.fee_destination.owner
            } else {
                destination.owner
            },
            input.context.claims_aggregate(),
            [0; 32],
        ),
    }
}

fn authenticate_sell(
    input: DirectComplementaryProjectionInputV2<'_>,
    destination: DirectExternalCollateralV2,
) -> Result<()> {
    if destination.account == [0; 32]
        || destination.owner == [0; 32]
        || destination.owner != input.record_before.maker()
        || destination.account != input.record_before.intent().collateral_account
        || destination.account == input.context.hoard_token_account
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}
