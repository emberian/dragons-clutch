//! Complete pure transition for a real buy end paired with a real sell end.
//!
//! Cash conversion remains owned by the owner-settlement accumulator. This
//! module moves only native Eggs and the exact reservation ledgers that own
//! them. The returned plan joins both Position prestates, both Reservation
//! prestates, both owner rows, and the receipt's independent end latches in one
//! atomic write set.

use crate::{
    prepare_account_receipt_end_v1, Amount, AuthenticatedOwnerSettlementAccountV1,
    AuthenticatedPositionV3, AuthenticatedSettlementReceiptEndV1, Error,
    OwnerSettlementReceiptAccountingPlanV1, PositionSettlementPoststateV3, Result,
    SettlementSideV1, MAX_ORDERS,
};

/// Maximum native Egg width carried by the pure General V2 transition.
pub const MAX_OUTCOMES: usize = 16;
/// Both real ends must be present on a direct receipt.
pub const DIRECT_RECEIPT_EXPECTED_END_MASK_V1: u8 = 3;

/// Frozen order family used to authenticate one receipt membership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OrderKindV1 {
    /// A scalar order over exactly one Egg outcome.
    Single = 0,
    /// A bounded native-Egg coefficient portfolio.
    Portfolio = 1,
}

/// Reservation ownership phase admitted by this transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ReservationStateV1 {
    /// Candidate finalization froze this order's complete entitlement.
    Entitled = 2,
    /// Every entitled unit and price unit was accounted exactly once.
    Consumed = 3,
}

/// Frozen order-set membership authenticated by the General V2 adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedOrderMembershipV1 {
    /// Market-runtime identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Digest of the complete ordered owner/order set.
    pub owner_order_set_digest: [u8; 32],
    /// Canonical order identity.
    pub order_id: [u8; 32],
    /// Canonical Reservation identity.
    pub reservation: [u8; 32],
    /// Semantic Position owner.
    pub owner: [u8; 32],
    /// Canonical order-set index.
    pub order_index: u8,
    /// Order generation frozen in the page row.
    pub order_generation: u64,
    /// Position generation frozen at placement.
    pub position_generation: u64,
    /// Buy or sell side.
    pub side: SettlementSideV1,
    /// Scalar or portfolio order family.
    pub order_kind: OrderKindV1,
    /// Active outcome width.
    pub outcome_count: u8,
    /// Scalar outcome, or `u8::MAX` for a portfolio.
    pub single_outcome: u8,
    /// Exact entitled Egg units across every selected slice for this order.
    pub entitled_units: Amount,
    /// Exact entitled consideration across every selected slice.
    pub entitled_consideration_price_units: u128,
}

impl AuthenticatedOrderMembershipV1 {
    pub(crate) fn validate(self) -> Result<()> {
        for key in [
            self.market,
            self.epoch,
            self.candidate,
            self.owner_order_set_digest,
            self.order_id,
            self.reservation,
            self.owner,
        ] {
            if key == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if usize::from(self.order_index) >= MAX_ORDERS
            || self.order_generation == 0
            || self.position_generation == 0
            || self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.entitled_units == 0
            || self.entitled_consideration_price_units == 0
        {
            return Err(Error::InvalidOrder);
        }
        match self.order_kind {
            OrderKindV1::Single if self.single_outcome >= self.outcome_count => {
                return Err(Error::InvalidOrder);
            }
            OrderKindV1::Portfolio if self.single_outcome != u8::MAX => {
                return Err(Error::InvalidOrder);
            }
            OrderKindV1::Single | OrderKindV1::Portfolio => {}
        }
        Ok(())
    }
}

/// Authenticated reservation projection with both cumulative ledgers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedReservationV1 {
    /// Canonical Reservation account.
    pub account: [u8; 32],
    /// Canonical content-derived Reservation identity.
    pub reservation: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Semantic owner.
    pub owner: [u8; 32],
    /// Canonical order identity.
    pub order_id: [u8; 32],
    /// Position account whose generation this reservation names.
    pub position: [u8; 32],
    /// Position generation frozen at placement.
    pub position_generation: u64,
    /// Order generation frozen at placement.
    pub order_generation: u64,
    /// Active outcome width.
    pub outcome_count: u8,
    /// Scalar or portfolio family.
    pub order_kind: OrderKindV1,
    /// Buy or sell side.
    pub side: SettlementSideV1,
    /// Entitled or consumed ownership state.
    pub state: ReservationStateV1,
    /// Initial buyer cash envelope; zero for sells.
    pub initial_cash_atoms: Amount,
    /// Cash still owned by this row before terminal handoff; zero for sells.
    pub remaining_cash_atoms: Amount,
    /// Initial seller Egg envelope; zero for buys.
    pub initial_internal: [Amount; MAX_OUTCOMES],
    /// Seller Egg envelope still awaiting delivery or return.
    pub remaining_internal: [Amount; MAX_OUTCOMES],
    /// Exact Egg units selected across all slices of this order.
    pub entitled_units: Amount,
    /// Egg units already delivered.
    pub consumed_units: Amount,
    /// Units whose consideration was recorded by the owner row.
    pub accounted_units: Amount,
    /// Exact selected consideration across all slices.
    pub entitled_consideration_price_units: u128,
    /// Exact consideration already recorded by the owner row.
    pub accounted_consideration_price_units: u128,
    /// Whether the account meta is writable.
    pub writable: bool,
}

impl AuthenticatedReservationV1 {
    pub(crate) fn validate(self) -> Result<()> {
        for key in [
            self.account,
            self.reservation,
            self.market,
            self.epoch,
            self.owner,
            self.order_id,
            self.position,
        ] {
            if key == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if self.account == self.position
            || self.account == self.reservation
            || self.account == self.owner
            || self.reservation == self.position
            || self.reservation == self.owner
            || self.position == self.owner
            || self.position_generation == 0
            || self.order_generation == 0
            || self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.entitled_units == 0
            || self.entitled_consideration_price_units == 0
            || self.consumed_units > self.entitled_units
            || self.accounted_units > self.entitled_units
            || self.accounted_consideration_price_units
                > self.entitled_consideration_price_units
            || !self.writable
            || !is_zero_padded(&self.initial_internal, self.outcome_count)
            || !is_zero_padded(&self.remaining_internal, self.outcome_count)
        {
            return Err(Error::InvalidAccount);
        }
        let delivery_complete = self.consumed_units == self.entitled_units;
        let accounting_complete = self.accounted_units == self.entitled_units;
        let accounting_value_complete = self.accounted_consideration_price_units
            == self.entitled_consideration_price_units;
        if accounting_complete != accounting_value_complete {
            return Err(Error::InvariantViolation);
        }
        let terminal = delivery_complete && accounting_complete;
        match self.state {
            ReservationStateV1::Entitled if terminal => return Err(Error::InvariantViolation),
            ReservationStateV1::Consumed if !terminal => {
                return Err(Error::InvariantViolation);
            }
            ReservationStateV1::Entitled | ReservationStateV1::Consumed => {}
        }
        match self.side {
            SettlementSideV1::Buy => {
                if self.initial_cash_atoms == 0
                    || self.initial_internal != [0; MAX_OUTCOMES]
                    || self.remaining_internal != [0; MAX_OUTCOMES]
                    || (!accounting_complete
                        && self.remaining_cash_atoms != self.initial_cash_atoms)
                    || (accounting_complete
                        && self.remaining_cash_atoms != 0)
                {
                    return Err(Error::InvariantViolation);
                }
            }
            SettlementSideV1::Sell => {
                if self.initial_cash_atoms != 0 || self.remaining_cash_atoms != 0 {
                    return Err(Error::InvariantViolation);
                }
                let initial = sum_internal(&self.initial_internal, self.outcome_count)?;
                let remaining = sum_internal(&self.remaining_internal, self.outcome_count)?;
                if initial == 0
                    || u128::from(self.entitled_units) > initial
                    || (self.state == ReservationStateV1::Entitled
                        && remaining
                            .checked_add(u128::from(self.consumed_units))
                            .ok_or(Error::ArithmeticOverflow)?
                            != initial)
                    || (self.state == ReservationStateV1::Consumed && remaining != 0)
                {
                    return Err(Error::InvariantViolation);
                }
            }
        }
        Ok(())
    }
}

/// One selected direct receipt authenticated from immutable candidate output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedDirectSettlementReceiptV1 {
    /// Canonical receipt account.
    pub receipt: [u8; 32],
    /// Earlier price-accounting transition identity.
    pub receipt_accounting_id: [u8; 32],
    /// Complete Egg-delivery transition identity.
    pub delivery_transition_id: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Digest of the complete ordered owner/order set.
    pub owner_order_set_digest: [u8; 32],
    /// Exact buyer order identity.
    pub buy_order_id: [u8; 32],
    /// Exact seller order identity.
    pub sell_order_id: [u8; 32],
    /// Exact native Egg outcome.
    pub outcome: u8,
    /// Exact native Egg quantity moved by this slice.
    pub quantity: Amount,
    /// Frozen scaled price for this outcome.
    pub price: Amount,
    /// Must equal `quantity * price` exactly.
    pub consideration_price_units: u128,
    /// Canonical zero-based selected-slice index.
    pub slice_index: u16,
    /// Must equal `slice_index + 1`.
    pub sequence: u64,
    /// Quantity already settled on this one-shot receipt; direct requires zero.
    pub settled_quantity: Amount,
    /// Price-accounting latches; delivery requires both already set.
    pub accounted_end_mask: u8,
    /// Independent Egg-delivery latches before this transition.
    pub delivered_end_mask: u8,
    /// Direct receipts require both real ends and no virtual end.
    pub expected_end_mask: u8,
}

impl AuthenticatedDirectSettlementReceiptV1 {
    fn validate_common(self, outcome_count: u8) -> Result<()> {
        for key in [
            self.receipt,
            self.receipt_accounting_id,
            self.delivery_transition_id,
            self.market,
            self.epoch,
            self.candidate,
            self.owner_order_set_digest,
            self.buy_order_id,
            self.sell_order_id,
        ] {
            if key == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if self.receipt_accounting_id == self.delivery_transition_id
            || self.buy_order_id == self.sell_order_id
            || self.outcome >= outcome_count
            || self.quantity == 0
            || self.price == 0
            || self.consideration_price_units
                != u128::from(self.quantity) * u128::from(self.price)
            || self.sequence != u64::from(self.slice_index) + 1
            || self.settled_quantity != 0
            || self.expected_end_mask != DIRECT_RECEIPT_EXPECTED_END_MASK_V1
            || self.accounted_end_mask & !self.expected_end_mask != 0
            || self.delivered_end_mask & !self.expected_end_mask != 0
        {
            return Err(Error::InvalidOrder);
        }
        Ok(())
    }

    fn validate_for_accounting(self, outcome_count: u8) -> Result<()> {
        self.validate_common(outcome_count)?;
        if self.accounted_end_mask == self.expected_end_mask || self.delivered_end_mask != 0 {
            return Err(Error::Terminal);
        }
        Ok(())
    }

    fn validate_for_delivery(self, outcome_count: u8) -> Result<()> {
        self.validate_common(outcome_count)?;
        if self.accounted_end_mask != self.expected_end_mask || self.delivered_end_mask != 0 {
            return Err(Error::AuthorityUnavailable);
        }
        Ok(())
    }
}

/// Complete authenticated prestate for one action-25 receipt end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DirectReceiptEndAccountingInputV1 {
    /// Immutable direct receipt before this end's accounting latch is consumed.
    pub receipt: AuthenticatedDirectSettlementReceiptV1,
    /// Frozen membership for exactly one real end.
    pub order: AuthenticatedOrderMembershipV1,
    /// Position projection authenticating owner and generation.
    pub position: AuthenticatedPositionV3,
    /// Reservation accounting prestate for this end.
    pub reservation: AuthenticatedReservationV1,
    /// Owner-settlement row prestate for this end.
    pub owner_row: AuthenticatedOwnerSettlementAccountV1,
}

/// Atomic one-end accounting poststate; it contains no Egg or cash movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DirectReceiptEndAccountingPlanV1 {
    /// Exact action-25 accounting replay identity.
    pub receipt_accounting_id: [u8; 32],
    /// Later delivery identity committed by the same selected receipt.
    pub delivery_transition_id: [u8; 32],
    /// Receipt account to write.
    pub receipt: [u8; 32],
    /// Monotone price-accounting latch mask after this end.
    pub receipt_accounted_end_mask: u8,
    /// Delivery remains independently unconsumed.
    pub receipt_delivered_end_mask: u8,
    /// Reservation accounting poststate.
    pub reservation: AuthenticatedReservationV1,
    /// Owner-row accounting poststate.
    pub owner_accounting: OwnerSettlementReceiptAccountingPlanV1,
    /// Whether this order is now completely accounted.
    pub accounting_complete: bool,
    /// Buy-reservation cash handed to the aggregate owner row exactly once.
    pub reserved_cash_handoff_atoms: Amount,
}

/// Stage one real end's exact price accounting without delivering Eggs.
pub fn prepare_direct_receipt_end_accounting_v1(
    input: DirectReceiptEndAccountingInputV1,
) -> Result<DirectReceiptEndAccountingPlanV1> {
    input.order.validate()?;
    input.position.validate()?;
    input.reservation.validate()?;
    let position_fields = input.position.semantic.fields();
    input
        .receipt
        .validate_for_accounting(position_fields.outcome_count)?;
    validate_order_binding(
        input.receipt,
        input.order,
        input.position,
        input.reservation,
    )?;
    validate_owner_row_binding(
        input.receipt,
        input.order,
        input.position,
        input.owner_row,
        0,
    )?;
    let side_mask = match input.order.side {
        SettlementSideV1::Buy => 1,
        SettlementSideV1::Sell => 2,
    };
    if input.receipt.accounted_end_mask & side_mask != 0
        || input.reservation.state != ReservationStateV1::Entitled
        || input.order.order_id
            != match input.order.side {
                SettlementSideV1::Buy => input.receipt.buy_order_id,
                SettlementSideV1::Sell => input.receipt.sell_order_id,
            }
        || (input.order.order_kind == OrderKindV1::Single
            && input.order.single_outcome != input.receipt.outcome)
        || (input.order.side == SettlementSideV1::Buy
            && (input.reservation.remaining_internal != [0; MAX_OUTCOMES]
                || position_fields.reserved_cash_atoms
                    < input.owner_row.accumulator.expectation.reserved_cash_atoms
                || input.reservation.initial_cash_atoms
                    > input.owner_row.accumulator.expectation.reserved_cash_atoms))
        || (input.order.side == SettlementSideV1::Sell
            && input.reservation.remaining_internal[usize::from(input.receipt.outcome)]
                < input.receipt.quantity)
        || aliases_accounting_accounts(
            input.receipt.receipt,
            input.position.account,
            input.reservation.account,
            input.owner_row.address,
        )
    {
        return Err(Error::InvalidAccount);
    }

    let (mut reservation, completes) = advance_reservation_accounting(
        input.reservation,
        input.receipt.quantity,
        input.receipt.consideration_price_units,
    )?;
    let handoff = if input.order.side == SettlementSideV1::Buy && completes {
        let handoff = reservation.remaining_cash_atoms;
        reservation.remaining_cash_atoms = 0;
        handoff
    } else {
        0
    };
    reservation.validate()?;

    let owner_accounting = prepare_account_receipt_end_v1(
        input.owner_row,
        accounting_receipt_end(input.receipt, input.order, completes),
    )?;
    if owner_accounting.receipt_accounting_id != input.receipt.receipt_accounting_id
        || owner_accounting.receipt != input.receipt.receipt
        || owner_accounting.receipt_accounted_end_mask
            != input.receipt.accounted_end_mask | side_mask
    {
        return Err(Error::InvariantViolation);
    }

    Ok(DirectReceiptEndAccountingPlanV1 {
        receipt_accounting_id: input.receipt.receipt_accounting_id,
        delivery_transition_id: input.receipt.delivery_transition_id,
        receipt: input.receipt.receipt,
        receipt_accounted_end_mask: owner_accounting.receipt_accounted_end_mask,
        receipt_delivered_end_mask: 0,
        reservation,
        owner_accounting,
        accounting_complete: completes,
        reserved_cash_handoff_atoms: handoff,
    })
}

/// Complete authenticated prestate for one already-accounted direct receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DirectEggSettlementInputV1 {
    /// Immutable direct receipt.
    pub receipt: AuthenticatedDirectSettlementReceiptV1,
    /// Frozen buyer membership.
    pub buyer_order: AuthenticatedOrderMembershipV1,
    /// Frozen seller membership.
    pub seller_order: AuthenticatedOrderMembershipV1,
    /// Buyer Position prestate.
    pub buyer_position: AuthenticatedPositionV3,
    /// Seller Position prestate.
    pub seller_position: AuthenticatedPositionV3,
    /// Buy Reservation prestate.
    pub buyer_reservation: AuthenticatedReservationV1,
    /// Sell Reservation prestate.
    pub seller_reservation: AuthenticatedReservationV1,
    /// Buyer owner row, already finalized by action 38.
    pub buyer_owner_row: AuthenticatedOwnerSettlementAccountV1,
    /// Seller owner row, already finalized by action 38.
    pub seller_owner_row: AuthenticatedOwnerSettlementAccountV1,
}

/// Audited economic result of one exact direct Egg movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DirectEggTransferAuditV1 {
    /// Exact outcome moved.
    pub outcome: u8,
    /// Exact Egg atoms moved to the buyer.
    pub quantity: Amount,
    /// Whether the buyer order reached its delivery total.
    pub buyer_completes: bool,
    /// Whether the seller order reached its delivery total.
    pub seller_completes: bool,
    /// Unfilled seller portfolio remainder returned at completion.
    pub seller_returned_internal: [Amount; MAX_OUTCOMES],
}

/// One atomic poststate bundle for the future General V2 SBF adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DirectEggSettlementPlanV1 {
    /// Exact action-26 delivery identity; adapter payload must match it.
    pub delivery_transition_id: [u8; 32],
    /// Accounting replay identity whose complete latch authorizes delivery.
    pub receipt_accounting_id: [u8; 32],
    /// Receipt account to write.
    pub receipt: [u8; 32],
    /// Receipt quantity after the one-shot move.
    pub receipt_settled_quantity: Amount,
    /// Both independently named delivery latches after the move.
    pub receipt_delivered_end_mask: u8,
    /// Buyer Position poststate; generation and replay sequence are preserved.
    pub buyer_position: PositionSettlementPoststateV3,
    /// Seller Position poststate; generation and replay sequence are preserved.
    pub seller_position: PositionSettlementPoststateV3,
    /// Buy Reservation cumulative and terminal poststate.
    pub buyer_reservation: AuthenticatedReservationV1,
    /// Sell Reservation cumulative and terminal poststate.
    pub seller_reservation: AuthenticatedReservationV1,
    /// Exact movement and terminal-remainder audit fields.
    pub audit: DirectEggTransferAuditV1,
}

/// Deliver one already-accounted direct receipt atomically.
///
/// Both owner rows must already be in their one-way action-38 terminal state.
/// This transition moves only native Eggs and delivery ledgers; receipt price
/// accounting and cash realization cannot be replayed through this API.
pub fn prepare_direct_egg_settlement_v1(
    input: DirectEggSettlementInputV1,
) -> Result<DirectEggSettlementPlanV1> {
    let buyer_fields = input.buyer_position.semantic.fields();
    let seller_fields = input.seller_position.semantic.fields();
    input
        .receipt
        .validate_for_delivery(buyer_fields.outcome_count)?;
    validate_direct_input_common(
        input.receipt,
        input.buyer_order,
        input.seller_order,
        input.buyer_position,
        input.seller_position,
        input.buyer_reservation,
        input.seller_reservation,
    )?;
    validate_owner_row_binding(
        input.receipt,
        input.buyer_order,
        input.buyer_position,
        input.buyer_owner_row,
        1,
    )?;
    validate_owner_row_binding(
        input.receipt,
        input.seller_order,
        input.seller_position,
        input.seller_owner_row,
        1,
    )?;

    if input.buyer_owner_row.address == input.seller_owner_row.address
        || aliases_direct_accounts(
            input.receipt.receipt,
            input.buyer_position.account,
            input.seller_position.account,
            input.buyer_reservation.account,
            input.seller_reservation.account,
            input.buyer_owner_row.address,
            input.seller_owner_row.address,
        )
    {
        return Err(Error::InvalidAccount);
    }

    let (next_buy_reservation, buyer_completes) = advance_reservation_delivery(
        input.buyer_reservation,
        input.receipt.quantity,
    )?;
    let (mut next_sell_reservation, seller_completes) = advance_reservation_delivery(
        input.seller_reservation,
        input.receipt.quantity,
    )?;
    let outcome = usize::from(input.receipt.outcome);
    next_sell_reservation.remaining_internal[outcome] = next_sell_reservation
        .remaining_internal[outcome]
        .checked_sub(input.receipt.quantity)
        .ok_or(Error::ArithmeticUnderflow)?;

    let mut buyer_internal = buyer_fields.native_eggs;
    buyer_internal[outcome] = buyer_internal[outcome]
        .checked_add(input.receipt.quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut seller_internal = seller_fields.native_eggs;
    let mut returned = [0_u64; MAX_OUTCOMES];
    if seller_completes {
        returned = next_sell_reservation.remaining_internal;
        let mut at = 0_usize;
        while at < MAX_OUTCOMES {
            seller_internal[at] = seller_internal[at]
                .checked_add(returned[at])
                .ok_or(Error::ArithmeticOverflow)?;
            next_sell_reservation.remaining_internal[at] = 0;
            at += 1;
        }
    }
    next_buy_reservation.validate()?;
    next_sell_reservation.validate()?;
    let next_buyer_position = input.buyer_position.settlement_poststate(
        buyer_fields.cash_atoms,
        buyer_fields.reserved_cash_atoms,
        buyer_internal,
    )?;
    let next_seller_position = input.seller_position.settlement_poststate(
        seller_fields.cash_atoms,
        seller_fields.reserved_cash_atoms,
        seller_internal,
    )?;

    Ok(DirectEggSettlementPlanV1 {
        delivery_transition_id: input.receipt.delivery_transition_id,
        receipt_accounting_id: input.receipt.receipt_accounting_id,
        receipt: input.receipt.receipt,
        receipt_settled_quantity: input.receipt.quantity,
        receipt_delivered_end_mask: DIRECT_RECEIPT_EXPECTED_END_MASK_V1,
        buyer_position: next_buyer_position,
        seller_position: next_seller_position,
        buyer_reservation: next_buy_reservation,
        seller_reservation: next_sell_reservation,
        audit: DirectEggTransferAuditV1 {
            outcome: input.receipt.outcome,
            quantity: input.receipt.quantity,
            buyer_completes,
            seller_completes,
            seller_returned_internal: returned,
        },
    })
}

fn validate_direct_input_common(
    receipt: AuthenticatedDirectSettlementReceiptV1,
    buyer_order: AuthenticatedOrderMembershipV1,
    seller_order: AuthenticatedOrderMembershipV1,
    buyer_position: AuthenticatedPositionV3,
    seller_position: AuthenticatedPositionV3,
    buyer_reservation: AuthenticatedReservationV1,
    seller_reservation: AuthenticatedReservationV1,
) -> Result<()> {
    buyer_order.validate()?;
    seller_order.validate()?;
    buyer_position.validate()?;
    seller_position.validate()?;
    buyer_reservation.validate()?;
    seller_reservation.validate()?;
    let buyer_fields = buyer_position.semantic.fields();
    let seller_fields = seller_position.semantic.fields();
    let count = buyer_fields.outcome_count;
    if seller_fields.outcome_count != count
        || buyer_order.outcome_count != count
        || seller_order.outcome_count != count
        || buyer_reservation.outcome_count != count
        || seller_reservation.outcome_count != count
        || buyer_order.side != SettlementSideV1::Buy
        || seller_order.side != SettlementSideV1::Sell
        || buyer_reservation.side != SettlementSideV1::Buy
        || seller_reservation.side != SettlementSideV1::Sell
        || buyer_reservation.state != ReservationStateV1::Entitled
        || seller_reservation.state != ReservationStateV1::Entitled
        || buyer_fields.owner == seller_fields.owner
        || buyer_position.account == seller_position.account
        || buyer_fields.replay_account == seller_fields.replay_account
        || buyer_reservation.account == seller_reservation.account
        || receipt.buy_order_id != buyer_order.order_id
        || receipt.sell_order_id != seller_order.order_id
        || (buyer_order.order_kind == OrderKindV1::Single
            && buyer_order.single_outcome != receipt.outcome)
        || (seller_order.order_kind == OrderKindV1::Single
            && seller_order.single_outcome != receipt.outcome)
        || buyer_reservation.remaining_internal != [0; MAX_OUTCOMES]
        || seller_reservation.remaining_internal[usize::from(receipt.outcome)] < receipt.quantity
    {
        return Err(Error::InvariantViolation);
    }
    validate_order_binding(receipt, buyer_order, buyer_position, buyer_reservation)?;
    validate_order_binding(receipt, seller_order, seller_position, seller_reservation)
}

fn validate_order_binding(
    receipt: AuthenticatedDirectSettlementReceiptV1,
    order: AuthenticatedOrderMembershipV1,
    position: AuthenticatedPositionV3,
    reservation: AuthenticatedReservationV1,
) -> Result<()> {
    let fields = position.semantic.fields();
    if order.market != receipt.market
        || order.epoch != receipt.epoch
        || order.candidate != receipt.candidate
        || order.owner_order_set_digest != receipt.owner_order_set_digest
        || order.owner != fields.owner.bytes()
        || order.order_id != reservation.order_id
        || order.reservation != reservation.reservation
        || order.order_generation != reservation.order_generation
        || order.position_generation != fields.generation
        || order.position_generation != reservation.position_generation
        || order.side != reservation.side
        || order.order_kind != reservation.order_kind
        || order.outcome_count != fields.outcome_count
        || order.outcome_count != reservation.outcome_count
        || order.entitled_units != reservation.entitled_units
        || order.entitled_consideration_price_units
            != reservation.entitled_consideration_price_units
        || reservation.market != receipt.market
        || reservation.epoch != receipt.epoch
        || reservation.owner != fields.owner.bytes()
        || reservation.position != position.account
        || position.general_market_runtime != receipt.market
    {
        return Err(Error::AuthorityUnavailable);
    }
    Ok(())
}

fn validate_owner_row_binding(
    receipt: AuthenticatedDirectSettlementReceiptV1,
    order: AuthenticatedOrderMembershipV1,
    position: AuthenticatedPositionV3,
    owner_row: AuthenticatedOwnerSettlementAccountV1,
    required_state: u8,
) -> Result<()> {
    owner_row.accumulator.validate()?;
    let fields = position.semantic.fields();
    let expectation = owner_row.accumulator.expectation;
    if owner_row.address == [0; 32]
        || owner_row.accumulator.state != required_state
        || expectation.market != receipt.market
        || expectation.epoch != receipt.epoch
        || expectation.candidate != receipt.candidate
        || expectation.owner_order_set_digest != receipt.owner_order_set_digest
        || expectation.owner != order.owner
        || expectation.owner != fields.owner.bytes()
    {
        return Err(Error::AuthorityUnavailable);
    }
    Ok(())
}

fn aliases_accounting_accounts(
    receipt: [u8; 32],
    position: [u8; 32],
    reservation: [u8; 32],
    owner_row: [u8; 32],
) -> bool {
    let accounts = [receipt, position, reservation, owner_row];
    let mut left = 0_usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            if accounts[left] == accounts[right] {
                return true;
            }
            right += 1;
        }
        left += 1;
    }
    false
}

fn aliases_direct_accounts(
    receipt: [u8; 32],
    buyer_position: [u8; 32],
    seller_position: [u8; 32],
    buyer_reservation: [u8; 32],
    seller_reservation: [u8; 32],
    buyer_owner_row: [u8; 32],
    seller_owner_row: [u8; 32],
) -> bool {
    let accounts = [
        receipt,
        buyer_position,
        seller_position,
        buyer_reservation,
        seller_reservation,
        buyer_owner_row,
        seller_owner_row,
    ];
    let mut left = 0_usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            if accounts[left] == accounts[right] {
                return true;
            }
            right += 1;
        }
        left += 1;
    }
    false
}

pub(crate) fn advance_reservation_accounting(
    reservation: AuthenticatedReservationV1,
    quantity: Amount,
    consideration_price_units: u128,
) -> Result<(AuthenticatedReservationV1, bool)> {
    let mut next = reservation;
    next.accounted_units = next
        .accounted_units
        .checked_add(quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    next.accounted_consideration_price_units = next
        .accounted_consideration_price_units
        .checked_add(consideration_price_units)
        .ok_or(Error::ArithmeticOverflow)?;
    if next.accounted_units > next.entitled_units
        || next.accounted_consideration_price_units > next.entitled_consideration_price_units
    {
        return Err(Error::TooManyFragments);
    }
    let units_complete = next.accounted_units == next.entitled_units;
    let value_complete = next.accounted_consideration_price_units
        == next.entitled_consideration_price_units;
    if units_complete != value_complete {
        return Err(Error::InvariantViolation);
    }
    if units_complete && next.consumed_units == next.entitled_units {
        next.state = ReservationStateV1::Consumed;
    }
    Ok((next, units_complete))
}

pub(crate) fn advance_reservation_delivery(
    reservation: AuthenticatedReservationV1,
    quantity: Amount,
) -> Result<(AuthenticatedReservationV1, bool)> {
    let mut next = reservation;
    next.consumed_units = next
        .consumed_units
        .checked_add(quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    if next.consumed_units > next.accounted_units {
        return Err(Error::AuthorityUnavailable);
    }
    let complete = next.consumed_units == next.entitled_units;
    if complete && next.accounted_units == next.entitled_units {
        next.state = ReservationStateV1::Consumed;
    }
    Ok((next, complete))
}

fn accounting_receipt_end(
    receipt: AuthenticatedDirectSettlementReceiptV1,
    order: AuthenticatedOrderMembershipV1,
    completes_order: bool,
) -> AuthenticatedSettlementReceiptEndV1 {
    AuthenticatedSettlementReceiptEndV1 {
        receipt: receipt.receipt,
        receipt_accounting_id: receipt.receipt_accounting_id,
        market: receipt.market,
        epoch: receipt.epoch,
        candidate: receipt.candidate,
        owner_order_set_digest: receipt.owner_order_set_digest,
        owner: order.owner,
        order_index: order.order_index,
        side: order.side,
        consideration_price_units: receipt.consideration_price_units,
        completes_order,
        slice_index: receipt.slice_index,
        sequence: receipt.sequence,
        accounted_end_mask: receipt.accounted_end_mask,
        expected_end_mask: receipt.expected_end_mask,
    }
}

fn is_zero_padded(values: &[Amount; MAX_OUTCOMES], outcome_count: u8) -> bool {
    let mut at = usize::from(outcome_count);
    while at < MAX_OUTCOMES {
        if values[at] != 0 {
            return false;
        }
        at += 1;
    }
    true
}

fn sum_internal(values: &[Amount; MAX_OUTCOMES], outcome_count: u8) -> Result<u128> {
    let mut total = 0_u128;
    let mut at = 0_usize;
    while at < usize::from(outcome_count) {
        total = total
            .checked_add(u128::from(values[at]))
            .ok_or(Error::ArithmeticOverflow)?;
        at += 1;
    }
    Ok(total)
}
