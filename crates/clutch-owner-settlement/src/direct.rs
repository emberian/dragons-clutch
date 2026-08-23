//! Complete pure transition for a real buy end paired with a real sell end.
//!
//! Cash conversion remains owned by the owner-settlement accumulator. This
//! module moves only native Eggs and the exact reservation ledgers that own
//! them. The returned plan joins both Position prestates, both Reservation
//! prestates, both owner rows, and the receipt's independent end latches in one
//! atomic write set.

use crate::{
    prepare_account_receipt_end_v1, Amount, AuthenticatedOwnerSettlementAccountV1,
    AuthenticatedSettlementReceiptEndV1, Error, OwnerSettlementReceiptAccountingPlanV1, Result,
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
    fn validate(self) -> Result<()> {
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

/// Authenticated Position and replay projection used by settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedPositionV1 {
    /// Canonical Position account.
    pub position: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Semantic owner.
    pub owner: [u8; 32],
    /// Nonzero Position generation.
    pub generation: u64,
    /// Canonical replay account for this Position generation.
    pub replay: [u8; 32],
    /// Replay sequence observed before permissionless settlement.
    ///
    /// Settlement preserves this value because it is receipt-authorized, not
    /// an owner-signed command.
    pub replay_sequence: u64,
    /// Total Position cash, including its aggregate reserved subset.
    pub cash_atoms: Amount,
    /// Aggregate reserved cash across live reservations and owner rows.
    pub reserved_cash_atoms: Amount,
    /// Free internal native-Egg balances.
    pub internal: [Amount; MAX_OUTCOMES],
    /// Active outcome width.
    pub outcome_count: u8,
    /// Zero for an open Position.
    pub close_state: u8,
    /// Whether the account meta is writable.
    pub writable: bool,
}

impl AuthenticatedPositionV1 {
    fn validate(self) -> Result<()> {
        if self.position == [0; 32]
            || self.market == [0; 32]
            || self.owner == [0; 32]
            || self.replay == [0; 32]
            || self.position == self.replay
            || self.position == self.owner
            || self.replay == self.owner
            || self.generation == 0
            || self.reserved_cash_atoms > self.cash_atoms
            || self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.close_state != 0
            || !self.writable
            || !is_zero_padded(&self.internal, self.outcome_count)
        {
            return Err(Error::InvalidAccount);
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
    fn validate(self) -> Result<()> {
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
            || self.accounted_units != self.consumed_units
            || self.consumed_units > self.entitled_units
            || self.accounted_consideration_price_units
                > self.entitled_consideration_price_units
            || !self.writable
            || !is_zero_padded(&self.initial_internal, self.outcome_count)
            || !is_zero_padded(&self.remaining_internal, self.outcome_count)
        {
            return Err(Error::InvalidAccount);
        }
        let units_complete = self.consumed_units == self.entitled_units;
        let value_complete = self.accounted_consideration_price_units
            == self.entitled_consideration_price_units;
        if units_complete != value_complete {
            return Err(Error::InvariantViolation);
        }
        match self.state {
            ReservationStateV1::Entitled if units_complete => return Err(Error::InvariantViolation),
            ReservationStateV1::Consumed if !units_complete => {
                return Err(Error::InvariantViolation);
            }
            ReservationStateV1::Entitled | ReservationStateV1::Consumed => {}
        }
        match self.side {
            SettlementSideV1::Buy => {
                if self.initial_cash_atoms == 0
                    || self.initial_internal != [0; MAX_OUTCOMES]
                    || self.remaining_internal != [0; MAX_OUTCOMES]
                    || (self.state == ReservationStateV1::Entitled
                        && self.remaining_cash_atoms != self.initial_cash_atoms)
                    || (self.state == ReservationStateV1::Consumed
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
    /// Complete transition identity authenticated by the future SBF adapter.
    pub settlement_transition_id: [u8; 32],
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
    /// Independent buy/sell end latches before this transition.
    pub consumed_end_mask: u8,
    /// Direct receipts require both real ends and no virtual end.
    pub expected_end_mask: u8,
}

impl AuthenticatedDirectSettlementReceiptV1 {
    fn validate(self, outcome_count: u8) -> Result<()> {
        for key in [
            self.receipt,
            self.settlement_transition_id,
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
        if self.buy_order_id == self.sell_order_id
            || self.outcome >= outcome_count
            || self.quantity == 0
            || self.price == 0
            || self.consideration_price_units
                != u128::from(self.quantity) * u128::from(self.price)
            || self.sequence != u64::from(self.slice_index) + 1
            || self.settled_quantity != 0
            || self.consumed_end_mask != 0
            || self.expected_end_mask != DIRECT_RECEIPT_EXPECTED_END_MASK_V1
        {
            return Err(Error::InvalidOrder);
        }
        Ok(())
    }
}

/// Complete authenticated prestate for one real-ended direct receipt.
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
    pub buyer_position: AuthenticatedPositionV1,
    /// Seller Position prestate.
    pub seller_position: AuthenticatedPositionV1,
    /// Buy Reservation prestate.
    pub buyer_reservation: AuthenticatedReservationV1,
    /// Sell Reservation prestate.
    pub seller_reservation: AuthenticatedReservationV1,
    /// Buyer owner-settlement row prestate.
    pub buyer_owner_row: AuthenticatedOwnerSettlementAccountV1,
    /// Seller owner-settlement row prestate.
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
    /// Exact scaled consideration recorded for each real end.
    pub consideration_price_units: u128,
    /// Whether the buyer order reached both cumulative totals.
    pub buyer_completes: bool,
    /// Whether the seller order reached both cumulative totals.
    pub seller_completes: bool,
    /// Buyer cash envelope handed from a completed Reservation to its owner row.
    pub buyer_reserved_cash_handoff_atoms: Amount,
    /// Unfilled seller portfolio remainder returned at completion.
    pub seller_returned_internal: [Amount; MAX_OUTCOMES],
}

/// One atomic poststate bundle for the future General V2 SBF adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DirectEggSettlementPlanV1 {
    /// Exact complete transition identity; adapter payload must match it.
    pub settlement_transition_id: [u8; 32],
    /// Receipt account to write.
    pub receipt: [u8; 32],
    /// Receipt quantity after the one-shot move.
    pub receipt_settled_quantity: Amount,
    /// Both independently named real-end latches after the move.
    pub receipt_consumed_end_mask: u8,
    /// Buyer Position poststate; generation and replay sequence are preserved.
    pub buyer_position: AuthenticatedPositionV1,
    /// Seller Position poststate; generation and replay sequence are preserved.
    pub seller_position: AuthenticatedPositionV1,
    /// Buy Reservation cumulative and terminal poststate.
    pub buyer_reservation: AuthenticatedReservationV1,
    /// Sell Reservation cumulative and terminal poststate.
    pub seller_reservation: AuthenticatedReservationV1,
    /// Buyer owner-row accounting poststate.
    pub buyer_owner_accounting: OwnerSettlementReceiptAccountingPlanV1,
    /// Seller owner-row accounting poststate.
    pub seller_owner_accounting: OwnerSettlementReceiptAccountingPlanV1,
    /// Exact movement and terminal-remainder audit fields.
    pub audit: DirectEggTransferAuditV1,
}

/// Stage a complete real-ended Egg/reservation/accounting transition.
///
/// This transition never converts or moves cash. It records exact price units
/// in both owner rows while moving the native Egg from the seller Reservation
/// to the buyer Position. A completing seller gets the entire unfilled vector
/// back. A completing buyer Reservation hands its cash-envelope ownership to
/// the already-frozen owner row while the Position's aggregate reserved-cash
/// field remains unchanged for terminal owner-level realization.
pub fn prepare_direct_egg_settlement_v1(
    input: DirectEggSettlementInputV1,
) -> Result<DirectEggSettlementPlanV1> {
    input.buyer_order.validate()?;
    input.seller_order.validate()?;
    input.buyer_position.validate()?;
    input.seller_position.validate()?;
    input.buyer_reservation.validate()?;
    input.seller_reservation.validate()?;
    let count = input.buyer_position.outcome_count;
    input.receipt.validate(count)?;

    if input.seller_position.outcome_count != count
        || input.buyer_order.outcome_count != count
        || input.seller_order.outcome_count != count
        || input.buyer_reservation.outcome_count != count
        || input.seller_reservation.outcome_count != count
        || input.buyer_order.side != SettlementSideV1::Buy
        || input.seller_order.side != SettlementSideV1::Sell
        || input.buyer_reservation.side != SettlementSideV1::Buy
        || input.seller_reservation.side != SettlementSideV1::Sell
        || input.buyer_reservation.state != ReservationStateV1::Entitled
        || input.seller_reservation.state != ReservationStateV1::Entitled
        || input.buyer_position.owner == input.seller_position.owner
        || input.buyer_position.position == input.seller_position.position
        || input.buyer_position.replay == input.seller_position.replay
        || input.buyer_reservation.account == input.seller_reservation.account
        || input.buyer_owner_row.address == input.seller_owner_row.address
        || aliases_writable_account(&input)
    {
        return Err(Error::InvalidAccount);
    }

    validate_common_binding(
        &input,
        input.buyer_order,
        input.buyer_position,
        input.buyer_reservation,
    )?;
    validate_common_binding(
        &input,
        input.seller_order,
        input.seller_position,
        input.seller_reservation,
    )?;
    if input.receipt.buy_order_id != input.buyer_order.order_id
        || input.receipt.sell_order_id != input.seller_order.order_id
        || (input.buyer_order.order_kind == OrderKindV1::Single
            && input.buyer_order.single_outcome != input.receipt.outcome)
        || (input.seller_order.order_kind == OrderKindV1::Single
            && input.seller_order.single_outcome != input.receipt.outcome)
        || input.buyer_reservation.remaining_internal != [0; MAX_OUTCOMES]
        || input.seller_reservation.remaining_internal[usize::from(input.receipt.outcome)]
            < input.receipt.quantity
        || input.buyer_position.reserved_cash_atoms
            < input.buyer_owner_row.accumulator.expectation.reserved_cash_atoms
        || input.buyer_reservation.initial_cash_atoms
            > input.buyer_owner_row.accumulator.expectation.reserved_cash_atoms
    {
        return Err(Error::InvariantViolation);
    }

    let (mut next_buy_reservation, buyer_completes) =
        advance_reservation(input.buyer_reservation, input.receipt)?;
    let (mut next_sell_reservation, seller_completes) =
        advance_reservation(input.seller_reservation, input.receipt)?;
    let outcome = usize::from(input.receipt.outcome);
    next_sell_reservation.remaining_internal[outcome] = next_sell_reservation
        .remaining_internal[outcome]
        .checked_sub(input.receipt.quantity)
        .ok_or(Error::ArithmeticUnderflow)?;

    let mut next_buyer_position = input.buyer_position;
    next_buyer_position.internal[outcome] = next_buyer_position.internal[outcome]
        .checked_add(input.receipt.quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut next_seller_position = input.seller_position;
    let mut returned = [0_u64; MAX_OUTCOMES];
    if seller_completes {
        returned = next_sell_reservation.remaining_internal;
        let mut at = 0_usize;
        while at < MAX_OUTCOMES {
            next_seller_position.internal[at] = next_seller_position.internal[at]
                .checked_add(returned[at])
                .ok_or(Error::ArithmeticOverflow)?;
            next_sell_reservation.remaining_internal[at] = 0;
            at += 1;
        }
    }
    let buyer_cash_handoff = if buyer_completes {
        let value = next_buy_reservation.remaining_cash_atoms;
        next_buy_reservation.remaining_cash_atoms = 0;
        value
    } else {
        0
    };
    next_buy_reservation.validate()?;
    next_sell_reservation.validate()?;
    next_buyer_position.validate()?;
    next_seller_position.validate()?;

    let buy_accounting = prepare_account_receipt_end_v1(
        input.buyer_owner_row,
        receipt_end(input.receipt, input.buyer_order, buyer_completes),
    )?;
    let sell_accounting = prepare_account_receipt_end_v1(
        input.seller_owner_row,
        receipt_end(input.receipt, input.seller_order, seller_completes),
    )?;
    if buy_accounting.settlement_transition_id != input.receipt.settlement_transition_id
        || sell_accounting.settlement_transition_id != input.receipt.settlement_transition_id
        || buy_accounting.receipt != input.receipt.receipt
        || sell_accounting.receipt != input.receipt.receipt
        || buy_accounting.receipt_consumed_end_mask != 1
        || sell_accounting.receipt_consumed_end_mask != 2
    {
        return Err(Error::InvariantViolation);
    }

    Ok(DirectEggSettlementPlanV1 {
        settlement_transition_id: input.receipt.settlement_transition_id,
        receipt: input.receipt.receipt,
        receipt_settled_quantity: input.receipt.quantity,
        receipt_consumed_end_mask: DIRECT_RECEIPT_EXPECTED_END_MASK_V1,
        buyer_position: next_buyer_position,
        seller_position: next_seller_position,
        buyer_reservation: next_buy_reservation,
        seller_reservation: next_sell_reservation,
        buyer_owner_accounting: buy_accounting,
        seller_owner_accounting: sell_accounting,
        audit: DirectEggTransferAuditV1 {
            outcome: input.receipt.outcome,
            quantity: input.receipt.quantity,
            consideration_price_units: input.receipt.consideration_price_units,
            buyer_completes,
            seller_completes,
            buyer_reserved_cash_handoff_atoms: buyer_cash_handoff,
            seller_returned_internal: returned,
        },
    })
}

fn validate_common_binding(
    input: &DirectEggSettlementInputV1,
    order: AuthenticatedOrderMembershipV1,
    position: AuthenticatedPositionV1,
    reservation: AuthenticatedReservationV1,
) -> Result<()> {
    let receipt = input.receipt;
    if order.market != receipt.market
        || order.epoch != receipt.epoch
        || order.candidate != receipt.candidate
        || order.owner_order_set_digest != receipt.owner_order_set_digest
        || order.owner != position.owner
        || order.order_id != reservation.order_id
        || order.reservation != reservation.reservation
        || order.order_generation != reservation.order_generation
        || order.position_generation != position.generation
        || order.position_generation != reservation.position_generation
        || order.side != reservation.side
        || order.order_kind != reservation.order_kind
        || order.outcome_count != position.outcome_count
        || order.outcome_count != reservation.outcome_count
        || order.entitled_units != reservation.entitled_units
        || order.entitled_consideration_price_units
            != reservation.entitled_consideration_price_units
        || reservation.market != receipt.market
        || reservation.epoch != receipt.epoch
        || reservation.owner != position.owner
        || reservation.position != position.position
        || position.market != receipt.market
    {
        return Err(Error::AuthorityUnavailable);
    }
    let owner_row = match order.side {
        SettlementSideV1::Buy => input.buyer_owner_row,
        SettlementSideV1::Sell => input.seller_owner_row,
    };
    let expectation = owner_row.accumulator.expectation;
    if expectation.market != receipt.market
        || expectation.epoch != receipt.epoch
        || expectation.candidate != receipt.candidate
        || expectation.owner_order_set_digest != receipt.owner_order_set_digest
        || expectation.owner != position.owner
    {
        return Err(Error::AuthorityUnavailable);
    }
    Ok(())
}

fn aliases_writable_account(input: &DirectEggSettlementInputV1) -> bool {
    let accounts = [
        input.receipt.receipt,
        input.buyer_position.position,
        input.seller_position.position,
        input.buyer_reservation.account,
        input.seller_reservation.account,
        input.buyer_owner_row.address,
        input.seller_owner_row.address,
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

fn advance_reservation(
    reservation: AuthenticatedReservationV1,
    receipt: AuthenticatedDirectSettlementReceiptV1,
) -> Result<(AuthenticatedReservationV1, bool)> {
    let mut next = reservation;
    next.consumed_units = next
        .consumed_units
        .checked_add(receipt.quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    next.accounted_units = next
        .accounted_units
        .checked_add(receipt.quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    next.accounted_consideration_price_units = next
        .accounted_consideration_price_units
        .checked_add(receipt.consideration_price_units)
        .ok_or(Error::ArithmeticOverflow)?;
    if next.consumed_units > next.entitled_units
        || next.accounted_consideration_price_units > next.entitled_consideration_price_units
    {
        return Err(Error::TooManyFragments);
    }
    let units_complete = next.consumed_units == next.entitled_units;
    let value_complete = next.accounted_consideration_price_units
        == next.entitled_consideration_price_units;
    if units_complete != value_complete {
        return Err(Error::InvariantViolation);
    }
    if units_complete {
        next.state = ReservationStateV1::Consumed;
    }
    Ok((next, units_complete))
}

fn receipt_end(
    receipt: AuthenticatedDirectSettlementReceiptV1,
    order: AuthenticatedOrderMembershipV1,
    completes_order: bool,
) -> AuthenticatedSettlementReceiptEndV1 {
    AuthenticatedSettlementReceiptEndV1 {
        receipt: receipt.receipt,
        settlement_transition_id: receipt.settlement_transition_id,
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
        consumed_end_mask: receipt.consumed_end_mask,
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
