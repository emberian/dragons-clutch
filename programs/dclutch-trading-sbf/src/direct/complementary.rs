//! Runtime-width Direct complementary Custody projection.
//!
//! Claims remains the sole owner of the affine N-position liability mutation.
//! This module projects only each already-checked participant's two canonical
//! Custody routes: principal/net first, then fee. It has no width specialization
//! and no caller-authored effect authority.

use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyRequestV1, OperationV1,
};
use dclutch_direct_codec::successor::{
    ComplementaryActionV2, ComplementarySettlementV2, DirectExecutionConfigV1,
    DirectRegisteredIntentV2, RegisteredFillCandidateV2, RegisteredRecordAfterFillV2,
};

use super::physical::{
    DirectExternalCollateralV2, DirectExternalDebitV2, DirectPhysicalError, Result,
};

/// Canonical route order for every complementary participant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectComplementaryCustodyRouteV2 {
    /// Split gross into Hoard principal, or merge net out to the seller.
    PrincipalOrNet,
    /// Charge the participant's cumulative-difference fee.
    Fee,
}

/// Exact fixed-role facts common to every complementary Custody effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryPhysicalContextV2 {
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
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
    /// Immutable Market generation.
    pub generation: u64,
}

impl DirectComplementaryPhysicalContextV2 {
    fn validate(self, config: DirectExecutionConfigV1) -> Result<()> {
        for identity in [
            self.trading_program,
            self.release_set,
            self.market,
            self.realm,
            self.mint,
            self.token_program,
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
        if self.fee_destination.owner != config.fee_recipient()
            || self.fee_destination.account == self.hoard_token_account
        {
            return Err(DirectPhysicalError::Binding);
        }
        Ok(())
    }
}

/// Side-specific authenticated participant collateral endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectComplementaryCollateralV2 {
    /// Registered Buy external source with exact Custody delegation.
    BuySource(DirectExternalDebitV2),
    /// Registered Sell external destination.
    SellDestination(DirectExternalCollateralV2),
}

/// Authenticated physical coordinates for one canonical outcome participant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryParticipantV2 {
    /// Exact Direct maker-root account and Custody replay context.
    pub maker_root: [u8; 32],
    /// Exact live registered-record account.
    pub record: [u8; 32],
    /// Side-specific collateral account observation.
    pub collateral: DirectComplementaryCollateralV2,
    /// Per-maker Custody replay revision before this participant's first effect.
    pub custody_replay_revision: u64,
}

/// Complete semantic input for one route and one canonical Product outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryProjectionInputV2 {
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
    pub participant: DirectComplementaryParticipantV2,
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
    /// Participant delegate allowance after both routes, for Buy only.
    pub terminal_delegated_amount: Option<u64>,
}

/// Aggregate token poststate checked before the first complementary CPI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectComplementaryCustodyAggregateV2 {
    /// HoardPrincipal balance after all principal/net routes.
    pub hoard_after: u64,
    /// Venue-fee destination balance after every fee route.
    pub fee_after: u64,
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
    input: DirectComplementaryProjectionInputV2,
) -> Result<Option<DirectComplementaryCustodyEffectV2>> {
    input.context.validate(input.config)?;
    authenticate_coordinate(input)?;
    let shape = effect_shape(input)?;
    if shape.amount == 0 {
        return Ok(None);
    }
    let principal_positive = match input.action {
        ComplementaryActionV2::Split => input.candidate.effects.gross_collateral_debit != 0,
        ComplementaryActionV2::Merge => input.candidate.effects.net_collateral_credit != 0,
    };
    let transfer_index = match input.route {
        DirectComplementaryCustodyRouteV2::PrincipalOrNet => 0,
        DirectComplementaryCustodyRouteV2::Fee if principal_positive => 1,
        DirectComplementaryCustodyRouteV2::Fee => 0,
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
        release_set: input.context.release_set,
        market: input.context.market,
        realm: input.context.realm,
        context: input.participant.maker_root,
        caller_program: input.context.trading_program,
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner,
            destination_owner,
            order: input.participant.record,
            parent_request_digest: input.context.parent_request_digest,
            order_nonce: input.record_before.intent().nonce,
            generation: input.context.generation,
            page_index: 0,
            execution_index: input.participant_index,
            transfer_index,
        },
        source: shape.source,
        destination: shape.destination,
        source_vault_context,
        destination_vault_context,
        mint: input.context.mint,
        token_program: input.context.token_program,
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
    let terminal_delegated_amount = match input.participant.collateral {
        DirectComplementaryCollateralV2::BuySource(source) => Some(
            source
                .delegated_amount
                .checked_sub(input.candidate.effects.gross_collateral_debit)
                .and_then(|value| value.checked_sub(input.candidate.effects.fee_transfer))
                .ok_or(DirectPhysicalError::Arithmetic)?,
        ),
        DirectComplementaryCollateralV2::SellDestination(_) => None,
    };
    Ok(Some(DirectComplementaryCustodyEffectV2 {
        request,
        terminal_delegated_amount,
    }))
}

fn authenticate_coordinate(input: DirectComplementaryProjectionInputV2) -> Result<()> {
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
        || input.candidate.maker_root.market() != input.context.market
        || input.candidate.maker_root.generation() != input.context.generation
        || intent.market != input.context.market
        || intent.generation != input.context.generation
        || intent.outcome != input.participant_index
        || candidate_nonce != intent.nonce
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

struct CustodyShapeV2 {
    source: [u8; 32],
    destination: [u8; 32],
    source_compartment: CompartmentV1,
    destination_compartment: CompartmentV1,
    amount: u64,
}

fn effect_shape(input: DirectComplementaryProjectionInputV2) -> Result<CustodyShapeV2> {
    match (input.action, input.route, input.participant.collateral) {
        (
            ComplementaryActionV2::Split,
            DirectComplementaryCustodyRouteV2::PrincipalOrNet,
            DirectComplementaryCollateralV2::BuySource(source),
        ) => {
            authenticate_buy(input, source)?;
            Ok(CustodyShapeV2 {
                source: source.account,
                destination: input.context.hoard_token_account,
                source_compartment: CompartmentV1::External,
                destination_compartment: CompartmentV1::HoardPrincipal,
                amount: input.candidate.effects.gross_collateral_debit,
            })
        }
        (
            ComplementaryActionV2::Split,
            DirectComplementaryCustodyRouteV2::Fee,
            DirectComplementaryCollateralV2::BuySource(source),
        ) => {
            authenticate_buy(input, source)?;
            Ok(CustodyShapeV2 {
                source: source.account,
                destination: input.context.fee_destination.account,
                source_compartment: CompartmentV1::External,
                destination_compartment: CompartmentV1::External,
                amount: input.candidate.effects.fee_transfer,
            })
        }
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
    input: DirectComplementaryProjectionInputV2,
) -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
    match input.participant.collateral {
        DirectComplementaryCollateralV2::BuySource(source) => (
            source.owner,
            if input.route == DirectComplementaryCustodyRouteV2::Fee {
                input.context.fee_destination.owner
            } else {
                [0; 32]
            },
            [0; 32],
            if input.route == DirectComplementaryCustodyRouteV2::PrincipalOrNet {
                input.context.market
            } else {
                [0; 32]
            },
        ),
        DirectComplementaryCollateralV2::SellDestination(destination) => (
            [0; 32],
            if input.route == DirectComplementaryCustodyRouteV2::Fee {
                input.context.fee_destination.owner
            } else {
                destination.owner
            },
            input.context.market,
            [0; 32],
        ),
    }
}

fn authenticate_buy(
    input: DirectComplementaryProjectionInputV2,
    source: DirectExternalDebitV2,
) -> Result<()> {
    let spent = input
        .candidate
        .effects
        .gross_collateral_debit
        .checked_add(input.candidate.effects.fee_transfer)
        .ok_or(DirectPhysicalError::Arithmetic)?;
    let residual = match input.candidate.record {
        RegisteredRecordAfterFillV2::Live(record) => record.reserved_collateral(),
        RegisteredRecordAfterFillV2::Closed(close) => close.collateral_refund,
    };
    if source.account == [0; 32]
        || source.owner == [0; 32]
        || source.delegate != input.context.custody_authority
        || source.owner != input.record_before.maker()
        || source.account != input.record_before.intent().collateral_account
        || source.delegated_amount
            != residual
                .checked_add(spent)
                .ok_or(DirectPhysicalError::Arithmetic)?
        || source.balance < spent
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn authenticate_sell(
    input: DirectComplementaryProjectionInputV2,
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
