//! Delivery-complete owner settlement successor.
//!
//! V4 keeps the exact 288-byte V3 rent class while making virtual-merge
//! delivery completeness an owner-row fact. The immutable expectation owns the
//! exact merge-delivery end count. The existing mutable count is phase-typed:
//! it counts accounted ends while accumulating, resets exactly once at
//! accounting completion, then counts merge deliveries until cash realization.
//! Earlier owner-row versions are never decoded through the V4 envelope.

use crate::{
    owner_credit_atoms, owner_debit_atoms, owner_rounding_residue_price_units, Amount,
    AuthenticatedPositionV3, Error, OwnerSettlementCreateFundingV1,
    PositionSettlementPoststateV3, PresentConsiderationV2, Result, SelectedOwnerFeeV1,
    SettlementCashPotV1, SettlementSideV1, AuthenticatedReservationHandoffV3,
    AuthenticatedSettlementReceiptEndV4, AuthenticatedSettlementReceiptEndV5,
    SettlementReceiptDataIdV4, MAX_ORDERS,
};
use clutch_retirement::{PositionAccountV3, PositionV3Fields};

/// Canonical General outer tag selecting an owner-settlement row.
pub const OWNER_SETTLEMENT_OUTER_TAG_V4: u8 = 0x81;
/// Fresh General outer version selecting only the V4 semantic codec.
pub const OWNER_SETTLEMENT_OUTER_VERSION_V4: u8 = 4;
/// Exact persisted V4 owner-settlement semantic body width.
pub const OWNER_SETTLEMENT_BODY_V4_BYTES: usize = 288;
/// Fresh PDA domain for delivery-complete owner rows.
pub const OWNER_SETTLEMENT_PDA_DOMAIN_V4: &[u8] = b"owner-settlement:v4";
/// Fresh domain for one exact finalized V4 owner-row body.
pub const OWNER_FINALIZED_ROW_DATA_ID_DOMAIN_V4: &[u8] =
    b"clutch:owner-finalized-row-data:v4";

const EXPECTED_BUY_PRESENT_V4: u8 = 1 << 0;
const EXPECTED_SELL_PRESENT_V4: u8 = 1 << 1;
const CONSUMED_BUY_PRESENT_V4: u8 = 1 << 2;
const CONSUMED_SELL_PRESENT_V4: u8 = 1 << 3;
const OWNER_SETTLEMENT_PRESENCE_MASK_V4: u8 = EXPECTED_BUY_PRESENT_V4
    | EXPECTED_SELL_PRESENT_V4
    | CONSUMED_BUY_PRESENT_V4
    | CONSUMED_SELL_PRESENT_V4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReceiptEndSemanticV4 {
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner_order_set_digest: [u8; 32],
    owner: [u8; 32],
    order_index: u8,
    side: SettlementSideV1,
    consideration_price_units: PresentConsiderationV2,
    completes_order: bool,
    reservation_handoff: Option<AuthenticatedReservationHandoffV3>,
}

impl ReceiptEndSemanticV4 {
    const fn from_v4(value: &AuthenticatedSettlementReceiptEndV4) -> Self {
        Self {
            market: value.market,
            epoch: value.epoch,
            candidate: value.candidate,
            owner_order_set_digest: value.owner_order_set_digest,
            owner: value.owner,
            order_index: value.order_index,
            side: value.side,
            consideration_price_units: value.consideration_price_units,
            completes_order: value.completes_order,
            reservation_handoff: value.reservation_handoff,
        }
    }

    const fn from_v5(value: &AuthenticatedSettlementReceiptEndV5) -> Self {
        Self {
            market: value.market,
            epoch: value.epoch,
            candidate: value.candidate,
            owner_order_set_digest: value.owner_order_set_digest,
            owner: value.owner,
            order_index: value.order_index,
            side: value.side,
            consideration_price_units: value.consideration_price_units,
            completes_order: value.completes_order,
            reservation_handoff: value.reservation_handoff,
        }
    }
}
const BUY_END_MASK: u8 = 1;
const SELL_END_MASK: u8 = 2;

/// Exact V4 accounting/delivery/finalization phase encoded in the owner row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OwnerSettlementStateV4 {
    /// At least one expected receipt end remains unaccounted.
    Accumulating = 0,
    /// Every exact receipt end and order completion has been accounted.
    AccountingComplete = 1,
    /// The exact handoff has been allocated through Position and FinalPot.
    Finalized = 2,
}

impl OwnerSettlementStateV4 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Accumulating),
            1 => Ok(Self::AccountingComplete),
            2 => Ok(Self::Finalized),
            _ => Err(Error::InvalidExpectation),
        }
    }
}

/// Immutable verifier-owned owner expectation with no cash summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementExpectationV4 {
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner: [u8; 32],
    owner_order_set_digest: [u8; 32],
    price_scale: Amount,
    expected_buy_order_mask: u64,
    expected_sell_order_mask: u64,
    expected_slice_count: u16,
    expected_merge_delivery_count: u16,
    expected_buy_price_units: PresentConsiderationV2,
    expected_sell_price_units: PresentConsiderationV2,
    selected_fee_atoms: Amount,
}

impl OwnerSettlementExpectationV4 {
    /// General MarketRuntime identity.
    pub const fn market(&self) -> [u8; 32] {
        self.market
    }

    /// Counted Epoch identity.
    pub const fn epoch(&self) -> [u8; 32] {
        self.epoch
    }

    /// Final selected candidate identity.
    pub const fn candidate(&self) -> [u8; 32] {
        self.candidate
    }

    /// Semantic Position owner.
    pub const fn owner(&self) -> [u8; 32] {
        self.owner
    }

    /// Digest of the exhaustive owner/order membership book.
    pub const fn owner_order_set_digest(&self) -> [u8; 32] {
        self.owner_order_set_digest
    }

    /// Exact collateral price scale.
    pub const fn price_scale(&self) -> Amount {
        self.price_scale
    }

    /// Expected filled buy-order mask.
    pub const fn expected_buy_order_mask(&self) -> u64 {
        self.expected_buy_order_mask
    }

    /// Expected filled sell-order mask.
    pub const fn expected_sell_order_mask(&self) -> u64 {
        self.expected_sell_order_mask
    }

    /// Exact expected real receipt-end count.
    pub const fn expected_slice_count(&self) -> u16 {
        self.expected_slice_count
    }

    /// Exact virtual-merge seller ends that must be delivered before cash realization.
    pub const fn expected_merge_delivery_count(&self) -> u16 {
        self.expected_merge_delivery_count
    }

    /// Explicitly present aggregate buy consideration, including zero.
    pub const fn expected_buy_price_units(&self) -> PresentConsiderationV2 {
        self.expected_buy_price_units
    }

    /// Explicitly present aggregate sell consideration, including zero.
    pub const fn expected_sell_price_units(&self) -> PresentConsiderationV2 {
        self.expected_sell_price_units
    }

    /// Already-selected owner fee in whole collateral atoms.
    pub const fn selected_fee_atoms(&self) -> Amount {
        self.selected_fee_atoms
    }

    /// Validate identities, explicit side presence, masks, and fee shape.
    pub fn validate(&self) -> Result<()> {
        self.expected_buy_price_units.validate()?;
        self.expected_sell_price_units.validate()?;
        let identities = [
            self.market,
            self.epoch,
            self.candidate,
            self.owner,
            self.owner_order_set_digest,
        ];
        let mut left = 0usize;
        while left < identities.len() {
            if identities[left] == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
            let mut right = left + 1;
            while right < identities.len() {
                if identities[left] == identities[right] {
                    return Err(Error::InvalidIdentity);
                }
                right += 1;
            }
            left += 1;
        }
        let has_buy = self.expected_buy_order_mask != 0;
        let has_sell = self.expected_sell_order_mask != 0;
        if self.price_scale == 0
            || self.expected_slice_count == 0
            || self.expected_merge_delivery_count > self.expected_slice_count
            || (self.expected_buy_order_mask & self.expected_sell_order_mask) != 0
            || (!has_buy && !has_sell)
            || self.expected_buy_price_units.present != has_buy
            || self.expected_sell_price_units.present != has_sell
            || (self.expected_merge_delivery_count != 0 && !has_sell)
            || (!has_buy && self.selected_fee_atoms != 0)
        {
            return Err(Error::InvalidExpectation);
        }
        Ok(())
    }

    fn expected_presence_bits(&self) -> u8 {
        (if self.expected_buy_price_units.present {
            EXPECTED_BUY_PRESENT_V4
        } else {
            0
        }) | if self.expected_sell_price_units.present {
            EXPECTED_SELL_PRESENT_V4
        } else {
            0
        }
    }
}

/// One filled selected order with no caller-authored cash summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VerifiedSettlementOrderV4 {
    /// Semantic Position owner.
    pub owner: [u8; 32],
    /// Canonical selected order index.
    pub order_index: u8,
    /// Payer or payee side.
    pub side: SettlementSideV1,
    /// Exact present aggregate consideration, including zero.
    pub consideration_price_units: PresentConsiderationV2,
    /// Exact real receipt-end count for this order.
    pub slice_count: u16,
    /// Exact subset of sell ends routed into virtual merge.
    pub merge_delivery_count: u16,
}

/// Exact pre-fee owner expectation derived from selected settlement rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementExpectationBasisV4 {
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner: [u8; 32],
    owner_order_set_digest: [u8; 32],
    price_scale: Amount,
    expected_buy_order_mask: u64,
    expected_sell_order_mask: u64,
    expected_slice_count: u16,
    expected_merge_delivery_count: u16,
    expected_buy_price_units: PresentConsiderationV2,
    expected_sell_price_units: PresentConsiderationV2,
}

impl OwnerSettlementExpectationBasisV4 {
    /// General MarketRuntime identity.
    pub const fn market(&self) -> [u8; 32] {
        self.market
    }

    /// Counted Epoch identity.
    pub const fn epoch(&self) -> [u8; 32] {
        self.epoch
    }

    /// Final candidate identity.
    pub const fn candidate(&self) -> [u8; 32] {
        self.candidate
    }

    /// Semantic owner.
    pub const fn owner(&self) -> [u8; 32] {
        self.owner
    }

    /// Exhaustive owner/order-set digest.
    pub const fn owner_order_set_digest(&self) -> [u8; 32] {
        self.owner_order_set_digest
    }

    /// Exact collateral price scale.
    pub const fn price_scale(&self) -> Amount {
        self.price_scale
    }

    /// Expected buy-order mask.
    pub const fn expected_buy_order_mask(&self) -> u64 {
        self.expected_buy_order_mask
    }

    /// Expected sell-order mask.
    pub const fn expected_sell_order_mask(&self) -> u64 {
        self.expected_sell_order_mask
    }

    /// Exact expected receipt-end count.
    pub const fn expected_slice_count(&self) -> u16 {
        self.expected_slice_count
    }

    /// Exact virtual-merge seller ends derived from the selected Feed.
    pub const fn expected_merge_delivery_count(&self) -> u16 {
        self.expected_merge_delivery_count
    }

    /// Present aggregate buy consideration.
    pub const fn expected_buy_price_units(&self) -> PresentConsiderationV2 {
        self.expected_buy_price_units
    }

    /// Present aggregate sell consideration.
    pub const fn expected_sell_price_units(&self) -> PresentConsiderationV2 {
        self.expected_sell_price_units
    }

    /// Bind the exact fee owner's result after fee selection.
    pub fn with_selected_fee(
        self,
        selected_fee: SelectedOwnerFeeV1,
    ) -> Result<OwnerSettlementExpectationV4> {
        if selected_fee.owner != self.owner {
            return Err(Error::InvalidIdentity);
        }
        let expectation = OwnerSettlementExpectationV4 {
            market: self.market,
            epoch: self.epoch,
            candidate: self.candidate,
            owner: self.owner,
            owner_order_set_digest: self.owner_order_set_digest,
            price_scale: self.price_scale,
            expected_buy_order_mask: self.expected_buy_order_mask,
            expected_sell_order_mask: self.expected_sell_order_mask,
            expected_slice_count: self.expected_slice_count,
            expected_merge_delivery_count: self.expected_merge_delivery_count,
            expected_buy_price_units: self.expected_buy_price_units,
            expected_sell_price_units: self.expected_sell_price_units,
            selected_fee_atoms: selected_fee.fee_atoms,
        };
        expectation.validate()?;
        Ok(expectation)
    }
}

/// Complete owner-sorted pre-fee V4 basis book.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementExpectationBasisBookV4 {
    rows: [Option<OwnerSettlementExpectationBasisV4>; MAX_ORDERS],
    owner_count: u16,
}

impl OwnerSettlementExpectationBasisBookV4 {
    /// Exact participating owner count.
    pub const fn owner_count(&self) -> u16 {
        self.owner_count
    }

    /// Return one active sorted row.
    pub fn row(&self, ordinal: u16) -> Option<OwnerSettlementExpectationBasisV4> {
        if ordinal < self.owner_count {
            self.rows[usize::from(ordinal)]
        } else {
            None
        }
    }

    /// Return the unique row for one semantic owner.
    pub fn row_for_owner(&self, owner: [u8; 32]) -> Option<OwnerSettlementExpectationBasisV4> {
        let mut index = 0usize;
        while index < usize::from(self.owner_count) {
            if let Some(row) = self.rows[index] {
                if row.owner == owner {
                    return Some(row);
                }
            }
            index += 1;
        }
        None
    }
}

/// Derive the exhaustive owner-sorted V4 pre-fee basis.
#[allow(clippy::too_many_arguments)]
pub fn build_owner_settlement_expectation_basis_book_v4(
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner_order_set_digest: [u8; 32],
    price_scale: Amount,
    orders: &[VerifiedSettlementOrderV4; MAX_ORDERS],
    order_len: u8,
) -> Result<OwnerSettlementExpectationBasisBookV4> {
    if market == [0; 32]
        || epoch == [0; 32]
        || candidate == [0; 32]
        || owner_order_set_digest == [0; 32]
        || price_scale == 0
        || order_len == 0
        || usize::from(order_len) > MAX_ORDERS
    {
        return Err(Error::InvalidExpectation);
    }
    let mut rows: [Option<OwnerSettlementExpectationBasisV4>; MAX_ORDERS] = [None; MAX_ORDERS];
    let mut owner_count = 0usize;
    let mut seen_order_mask = 0u64;
    let mut order_at = 0usize;
    while order_at < usize::from(order_len) {
        let order = orders[order_at];
        order.consideration_price_units.validate()?;
        if order.owner == [0; 32]
            || !order.consideration_price_units.present
            || order.slice_count == 0
            || order.merge_delivery_count > order.slice_count
            || (order.side == SettlementSideV1::Buy && order.merge_delivery_count != 0)
            || usize::from(order.order_index) >= MAX_ORDERS
        {
            return Err(Error::InvalidOrder);
        }
        let bit = order_bit(order.order_index)?;
        if seen_order_mask & bit != 0 {
            return Err(Error::InvalidOrder);
        }
        seen_order_mask |= bit;
        let mut slot = 0usize;
        while slot < owner_count && rows[slot].map(|row| row.owner) != Some(order.owner) {
            slot += 1;
        }
        if slot == owner_count {
            if owner_count >= MAX_ORDERS {
                return Err(Error::ArithmeticOverflow);
            }
            rows[slot] = Some(OwnerSettlementExpectationBasisV4 {
                market,
                epoch,
                candidate,
                owner: order.owner,
                owner_order_set_digest,
                price_scale,
                expected_buy_order_mask: 0,
                expected_sell_order_mask: 0,
                expected_slice_count: 0,
                expected_merge_delivery_count: 0,
                expected_buy_price_units: PresentConsiderationV2::ABSENT,
                expected_sell_price_units: PresentConsiderationV2::ABSENT,
            });
            owner_count += 1;
        }
        let mut row = rows[slot].ok_or(Error::InvariantViolation)?;
        row.expected_slice_count = row
            .expected_slice_count
            .checked_add(order.slice_count)
            .ok_or(Error::ArithmeticOverflow)?;
        row.expected_merge_delivery_count = row
            .expected_merge_delivery_count
            .checked_add(order.merge_delivery_count)
            .ok_or(Error::ArithmeticOverflow)?;
        match order.side {
            SettlementSideV1::Buy => {
                row.expected_buy_order_mask |= bit;
                row.expected_buy_price_units.present = true;
                row.expected_buy_price_units.value = row
                    .expected_buy_price_units
                    .value
                    .checked_add(order.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            SettlementSideV1::Sell => {
                row.expected_sell_order_mask |= bit;
                row.expected_sell_price_units.present = true;
                row.expected_sell_price_units.value = row
                    .expected_sell_price_units
                    .value
                    .checked_add(order.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }
        rows[slot] = Some(row);
        order_at += 1;
    }
    let mut at = 1usize;
    while at < owner_count {
        let value = rows[at].ok_or(Error::InvariantViolation)?;
        let mut insert = at;
        while insert > 0
            && value.owner < rows[insert - 1].ok_or(Error::InvariantViolation)?.owner
        {
            rows[insert] = rows[insert - 1];
            insert -= 1;
        }
        rows[insert] = Some(value);
        at += 1;
    }
    let mut index = 0usize;
    while index < owner_count {
        let basis = rows[index].ok_or(Error::InvariantViolation)?;
        basis
            .with_selected_fee(SelectedOwnerFeeV1 {
                owner: basis.owner,
                fee_atoms: 0,
            })?
            .validate()?;
        index += 1;
    }
    Ok(OwnerSettlementExpectationBasisBookV4 {
        rows,
        owner_count: u16::try_from(owner_count).map_err(|_| Error::ArithmeticOverflow)?,
    })
}

/// Mutable V4 row with exact Reservation cash and merge-delivery ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementAccumulatorV4 {
    expectation: OwnerSettlementExpectationV4,
    buy_cash_handoff_atoms: Amount,
    consumed_buy_price_units: PresentConsiderationV2,
    consumed_sell_price_units: PresentConsiderationV2,
    completed_buy_order_mask: u64,
    completed_sell_order_mask: u64,
    consumed_slice_count: u16,
    state: OwnerSettlementStateV4,
}

impl OwnerSettlementAccumulatorV4 {
    /// Create a pristine row. Reservation cash is always initially zero.
    pub fn new(expectation: OwnerSettlementExpectationV4) -> Result<Self> {
        expectation.validate()?;
        Ok(Self {
            expectation,
            buy_cash_handoff_atoms: 0,
            consumed_buy_price_units: PresentConsiderationV2::ABSENT,
            consumed_sell_price_units: PresentConsiderationV2::ABSENT,
            completed_buy_order_mask: 0,
            completed_sell_order_mask: 0,
            consumed_slice_count: 0,
            state: OwnerSettlementStateV4::Accumulating,
        })
    }

    /// Immutable selected expectation.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV4 {
        self.expectation
    }

    /// Exact cash handed off by terminal buy Reservations so far.
    pub const fn buy_cash_handoff_atoms(&self) -> Amount {
        self.buy_cash_handoff_atoms
    }

    /// Exact consumed buy consideration.
    pub const fn consumed_buy_price_units(&self) -> PresentConsiderationV2 {
        self.consumed_buy_price_units
    }

    /// Exact consumed sell consideration.
    pub const fn consumed_sell_price_units(&self) -> PresentConsiderationV2 {
        self.consumed_sell_price_units
    }

    /// Completed buy-order mask.
    pub const fn completed_buy_order_mask(&self) -> u64 {
        self.completed_buy_order_mask
    }

    /// Completed sell-order mask.
    pub const fn completed_sell_order_mask(&self) -> u64 {
        self.completed_sell_order_mask
    }

    /// Phase-typed progress: accounted ends while accumulating, then delivered merge ends.
    pub const fn progress_count(&self) -> u16 {
        self.consumed_slice_count
    }

    /// Delivered virtual-merge ends after accounting has completed.
    pub fn merge_delivered_count(&self) -> Result<u16> {
        if self.state == OwnerSettlementStateV4::Accumulating {
            return Err(Error::Incomplete);
        }
        Ok(self.consumed_slice_count)
    }

    /// Exact accounting/finalization phase.
    pub const fn state(&self) -> OwnerSettlementStateV4 {
        self.state
    }

    /// Consume one receipt end and its inseparable terminal-buy cash handoff.
    pub fn consume(&mut self, receipt: &AuthenticatedSettlementReceiptEndV4) -> Result<()> {
        receipt.validate()?;
        self.consume_semantic(ReceiptEndSemanticV4::from_v4(receipt))
    }

    /// Consume one fresh rent-owned V5 receipt end through the same sole V4
    /// arithmetic state machine.
    pub fn consume_v5(&mut self, receipt: &AuthenticatedSettlementReceiptEndV5) -> Result<()> {
        receipt.validate()?;
        self.consume_semantic(ReceiptEndSemanticV4::from_v5(receipt))
    }

    fn consume_semantic(&mut self, receipt: ReceiptEndSemanticV4) -> Result<()> {
        self.validate()?;
        if self.state != OwnerSettlementStateV4::Accumulating {
            return Err(Error::Terminal);
        }
        let expected = self.expectation;
        if receipt.market != expected.market
            || receipt.epoch != expected.epoch
            || receipt.candidate != expected.candidate
            || receipt.owner_order_set_digest != expected.owner_order_set_digest
            || receipt.owner != expected.owner
        {
            return Err(Error::AuthorityUnavailable);
        }
        let bit = order_bit(receipt.order_index)?;
        let mut next = *self;
        match receipt.side {
            SettlementSideV1::Buy => {
                if expected.expected_buy_order_mask & bit == 0 {
                    return Err(Error::InvalidOrder);
                }
                next.consumed_buy_price_units.present = true;
                next.consumed_buy_price_units.value = next
                    .consumed_buy_price_units
                    .value
                    .checked_add(receipt.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
                if next.consumed_buy_price_units.value > expected.expected_buy_price_units.value {
                    return Err(Error::InvariantViolation);
                }
                if receipt.completes_order {
                    if next.completed_buy_order_mask & bit != 0 {
                        return Err(Error::DuplicateCompletion);
                    }
                    let handoff = receipt
                        .reservation_handoff
                        .ok_or(Error::AuthorityUnavailable)?;
                    next.buy_cash_handoff_atoms = next
                        .buy_cash_handoff_atoms
                        .checked_add(handoff.cash_atoms())
                        .ok_or(Error::ArithmeticOverflow)?;
                    next.completed_buy_order_mask |= bit;
                }
            }
            SettlementSideV1::Sell => {
                if expected.expected_sell_order_mask & bit == 0 {
                    return Err(Error::InvalidOrder);
                }
                next.consumed_sell_price_units.present = true;
                next.consumed_sell_price_units.value = next
                    .consumed_sell_price_units
                    .value
                    .checked_add(receipt.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
                if next.consumed_sell_price_units.value > expected.expected_sell_price_units.value {
                    return Err(Error::InvariantViolation);
                }
                if receipt.completes_order {
                    if next.completed_sell_order_mask & bit != 0 {
                        return Err(Error::DuplicateCompletion);
                    }
                    next.completed_sell_order_mask |= bit;
                }
            }
        }
        next.consumed_slice_count = next
            .consumed_slice_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if next.consumed_slice_count > expected.expected_slice_count {
            return Err(Error::TooManyFragments);
        }
        if next.accounting_fields_complete_with_count() {
            let required = owner_debit_atoms(
                expected.expected_buy_price_units.value,
                expected.price_scale,
                expected.selected_fee_atoms,
            )?;
            if next.buy_cash_handoff_atoms < required {
                return Err(Error::InsufficientCash);
            }
            next.consumed_slice_count = 0;
            next.state = OwnerSettlementStateV4::AccountingComplete;
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    fn accounting_totals_complete(&self) -> bool {
        self.consumed_buy_price_units == self.expectation.expected_buy_price_units
            && self.consumed_sell_price_units == self.expectation.expected_sell_price_units
            && self.completed_buy_order_mask == self.expectation.expected_buy_order_mask
            && self.completed_sell_order_mask == self.expectation.expected_sell_order_mask
    }

    fn accounting_fields_complete_with_count(&self) -> bool {
        self.consumed_slice_count == self.expectation.expected_slice_count
            && self.accounting_totals_complete()
    }

    /// Structurally advance one exact virtual-merge delivery.
    ///
    /// This count mutation grants no account authority. The General action-37
    /// composer must rederive it from the authenticated Receipt V4 delivery
    /// prestate and commit the row successor in the same atomic bundle.
    pub fn record_merge_delivery(&mut self) -> Result<()> {
        self.validate()?;
        if self.state != OwnerSettlementStateV4::AccountingComplete
            || self.consumed_slice_count >= self.expectation.expected_merge_delivery_count
        {
            return Err(Error::Incomplete);
        }
        let mut next = *self;
        next.consumed_slice_count = next
            .consumed_slice_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Convert the exact handoff once at the named Floor/Ceil boundary.
    pub fn finalize(
        &mut self,
        position_cash_atoms: Amount,
        position_reserved_cash_atoms: Amount,
    ) -> Result<OwnerSettlementDispositionV4> {
        self.validate()?;
        if self.state != OwnerSettlementStateV4::AccountingComplete {
            return Err(if self.state == OwnerSettlementStateV4::Finalized {
                Error::Terminal
            } else {
                Error::Incomplete
            });
        }
        if self.consumed_slice_count != self.expectation.expected_merge_delivery_count {
            return Err(Error::Incomplete);
        }
        let consideration_debit_atoms = owner_debit_atoms(
            self.expectation.expected_buy_price_units.value,
            self.expectation.price_scale,
            0,
        )?;
        let total_debit_atoms = consideration_debit_atoms
            .checked_add(self.expectation.selected_fee_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.buy_cash_handoff_atoms < total_debit_atoms
            || position_reserved_cash_atoms < self.buy_cash_handoff_atoms
            || position_cash_atoms < self.buy_cash_handoff_atoms
        {
            return Err(Error::InsufficientCash);
        }
        let credit_atoms = owner_credit_atoms(
            self.expectation.expected_sell_price_units.value,
            self.expectation.price_scale,
        )?;
        let residue_price_units = owner_rounding_residue_price_units(
            self.expectation.expected_buy_price_units.value,
            self.expectation.expected_sell_price_units.value,
            self.expectation.price_scale,
        )?;
        let released_cash_atoms = self
            .buy_cash_handoff_atoms
            .checked_sub(total_debit_atoms)
            .ok_or(Error::InsufficientCash)?;
        let position_cash_without_handoff = position_cash_atoms
            .checked_sub(self.buy_cash_handoff_atoms)
            .ok_or(Error::ArithmeticUnderflow)?;
        let position_cash_atoms = position_cash_without_handoff
            .checked_add(released_cash_atoms)
            .and_then(|value| value.checked_add(credit_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        let position_reserved_cash_atoms = position_reserved_cash_atoms
            .checked_sub(self.buy_cash_handoff_atoms)
            .ok_or(Error::ArithmeticUnderflow)?;
        if position_reserved_cash_atoms > position_cash_atoms {
            return Err(Error::InvariantViolation);
        }
        let disposition = OwnerSettlementDispositionV4 {
            buy_cash_handoff_atoms: self.buy_cash_handoff_atoms,
            consideration_debit_atoms,
            selected_fee_atoms: self.expectation.selected_fee_atoms,
            total_debit_atoms,
            credit_atoms,
            released_cash_atoms,
            residue_price_units,
            position_cash_atoms,
            position_reserved_cash_atoms,
        };
        self.state = OwnerSettlementStateV4::Finalized;
        self.validate()?;
        Ok(disposition)
    }

    /// Validate canonical presence, exact phase, and monotone progress.
    pub fn validate(&self) -> Result<()> {
        self.expectation.validate()?;
        self.consumed_buy_price_units.validate()?;
        self.consumed_sell_price_units.validate()?;
        if self.completed_buy_order_mask & !self.expectation.expected_buy_order_mask != 0
            || self.completed_sell_order_mask & !self.expectation.expected_sell_order_mask != 0
            || (self.consumed_buy_price_units.present
                && !self.expectation.expected_buy_price_units.present)
            || (self.consumed_sell_price_units.present
                && !self.expectation.expected_sell_price_units.present)
            || self.consumed_buy_price_units.value
                > self.expectation.expected_buy_price_units.value
            || self.consumed_sell_price_units.value
                > self.expectation.expected_sell_price_units.value
            || (!self.consumed_buy_price_units.present && self.completed_buy_order_mask != 0)
            || (!self.consumed_sell_price_units.present && self.completed_sell_order_mask != 0)
            || (self.state == OwnerSettlementStateV4::Accumulating
                && self.consumed_slice_count == 0
                && (self.consumed_buy_price_units.present
                    || self.consumed_sell_price_units.present))
            || (self.completed_buy_order_mask == 0 && self.buy_cash_handoff_atoms != 0)
            || (self.expectation.expected_buy_order_mask == 0 && self.buy_cash_handoff_atoms != 0)
        {
            return Err(Error::InvariantViolation);
        }
        let observed_side_count = (if self.consumed_buy_price_units.present {
            1u16
        } else {
            0
        })
            .checked_add(if self.consumed_sell_price_units.present {
                1u16
            } else {
                0
            })
            .ok_or(Error::ArithmeticOverflow)?;
        let accounting_complete = self.accounting_totals_complete();
        match self.state {
            OwnerSettlementStateV4::Accumulating => {
                if self.consumed_slice_count >= self.expectation.expected_slice_count
                    || observed_side_count > self.consumed_slice_count
                    || accounting_complete
                {
                    return Err(Error::InvariantViolation);
                }
            }
            OwnerSettlementStateV4::AccountingComplete => {
                if !accounting_complete
                    || self.consumed_slice_count > self.expectation.expected_merge_delivery_count
                {
                    return Err(Error::InvariantViolation);
                }
            }
            OwnerSettlementStateV4::Finalized => {
                if !accounting_complete
                    || self.consumed_slice_count != self.expectation.expected_merge_delivery_count
                {
                    return Err(Error::InvariantViolation);
                }
            }
        }
        if accounting_complete {
            let required = owner_debit_atoms(
                self.expectation.expected_buy_price_units.value,
                self.expectation.price_scale,
                self.expectation.selected_fee_atoms,
            )?;
            if self.buy_cash_handoff_atoms < required {
                return Err(Error::InsufficientCash);
            }
        }
        Ok(())
    }

    fn presence_bits(&self) -> u8 {
        self.expectation.expected_presence_bits()
            | if self.consumed_buy_price_units.present {
                CONSUMED_BUY_PRESENT_V4
            } else {
                0
            }
            | if self.consumed_sell_price_units.present {
                CONSUMED_SELL_PRESENT_V4
            } else {
                0
            }
    }

    /// Encode the exact canonical 288-byte V4 body.
    pub fn encode_body(&self) -> Result<[u8; OWNER_SETTLEMENT_BODY_V4_BYTES]> {
        self.validate()?;
        let mut output = [0u8; OWNER_SETTLEMENT_BODY_V4_BYTES];
        let mut cursor = 0usize;
        for identity in [
            self.expectation.market,
            self.expectation.epoch,
            self.expectation.candidate,
            self.expectation.owner,
            self.expectation.owner_order_set_digest,
        ] {
            put(&mut output, &mut cursor, &identity)?;
        }
        for value in [
            self.expectation.price_scale,
            self.expectation.expected_buy_order_mask,
            self.expectation.expected_sell_order_mask,
        ] {
            put(&mut output, &mut cursor, &value.to_le_bytes())?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_slice_count.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_buy_price_units.value.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_sell_price_units.value.to_le_bytes(),
        )?;
        for value in [
            self.expectation.selected_fee_atoms,
            self.buy_cash_handoff_atoms,
        ] {
            put(&mut output, &mut cursor, &value.to_le_bytes())?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.consumed_buy_price_units.value.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.consumed_sell_price_units.value.to_le_bytes(),
        )?;
        for value in [
            self.completed_buy_order_mask,
            self.completed_sell_order_mask,
        ] {
            put(&mut output, &mut cursor, &value.to_le_bytes())?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.consumed_slice_count.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &[self.state as u8, self.presence_bits()],
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_merge_delivery_count.to_le_bytes(),
        )?;
        if cursor != OWNER_SETTLEMENT_BODY_V4_BYTES {
            return Err(Error::InvariantViolation);
        }
        Ok(output)
    }

    /// Decode hostile bytes only after authenticating the exact `0x81/4` outer.
    pub fn decode_body(outer_tag: u8, outer_version: u8, input: &[u8]) -> Result<Self> {
        if outer_tag != OWNER_SETTLEMENT_OUTER_TAG_V4
            || outer_version != OWNER_SETTLEMENT_OUTER_VERSION_V4
            || input.len() != OWNER_SETTLEMENT_BODY_V4_BYTES
        {
            return Err(Error::InvalidAccount);
        }
        decode_semantic_body(input)
    }

    /// Project one exact finalized row for typed terminal joins.
    pub fn terminal_projection(&self) -> Result<OwnerSettlementTerminalProjectionV4> {
        self.validate()?;
        if self.state != OwnerSettlementStateV4::Finalized {
            return Err(Error::Incomplete);
        }
        Ok(OwnerSettlementTerminalProjectionV4 {
            expectation: self.expectation,
            finalized_body: self.encode_body()?,
        })
    }
}

fn decode_semantic_body(input: &[u8]) -> Result<OwnerSettlementAccumulatorV4> {
    if input.len() != OWNER_SETTLEMENT_BODY_V4_BYTES {
        return Err(Error::InvalidExpectation);
    }
    let mut cursor = 0usize;
    let market = read_key(input, &mut cursor)?;
    let epoch = read_key(input, &mut cursor)?;
    let candidate = read_key(input, &mut cursor)?;
    let owner = read_key(input, &mut cursor)?;
    let owner_order_set_digest = read_key(input, &mut cursor)?;
    let price_scale = read_u64(input, &mut cursor)?;
    let expected_buy_order_mask = read_u64(input, &mut cursor)?;
    let expected_sell_order_mask = read_u64(input, &mut cursor)?;
    let expected_slice_count = read_u16(input, &mut cursor)?;
    let expected_buy_value = read_u128(input, &mut cursor)?;
    let expected_sell_value = read_u128(input, &mut cursor)?;
    let selected_fee_atoms = read_u64(input, &mut cursor)?;
    let buy_cash_handoff_atoms = read_u64(input, &mut cursor)?;
    let consumed_buy_value = read_u128(input, &mut cursor)?;
    let consumed_sell_value = read_u128(input, &mut cursor)?;
    let completed_buy_order_mask = read_u64(input, &mut cursor)?;
    let completed_sell_order_mask = read_u64(input, &mut cursor)?;
    let consumed_slice_count = read_u16(input, &mut cursor)?;
    let state = OwnerSettlementStateV4::decode(read_u8(input, &mut cursor)?)?;
    let presence = read_u8(input, &mut cursor)?;
    let expected_merge_delivery_count = read_u16(input, &mut cursor)?;
    if presence & !OWNER_SETTLEMENT_PRESENCE_MASK_V4 != 0 || cursor != input.len() {
        return Err(Error::InvalidExpectation);
    }
    let value = OwnerSettlementAccumulatorV4 {
        expectation: OwnerSettlementExpectationV4 {
            market,
            epoch,
            candidate,
            owner,
            owner_order_set_digest,
            price_scale,
            expected_buy_order_mask,
            expected_sell_order_mask,
            expected_slice_count,
            expected_merge_delivery_count,
            expected_buy_price_units: PresentConsiderationV2 {
                present: presence & EXPECTED_BUY_PRESENT_V4 != 0,
                value: expected_buy_value,
            },
            expected_sell_price_units: PresentConsiderationV2 {
                present: presence & EXPECTED_SELL_PRESENT_V4 != 0,
                value: expected_sell_value,
            },
            selected_fee_atoms,
        },
        buy_cash_handoff_atoms,
        consumed_buy_price_units: PresentConsiderationV2 {
            present: presence & CONSUMED_BUY_PRESENT_V4 != 0,
            value: consumed_buy_value,
        },
        consumed_sell_price_units: PresentConsiderationV2 {
            present: presence & CONSUMED_SELL_PRESENT_V4 != 0,
            value: consumed_sell_value,
        },
        completed_buy_order_mask,
        completed_sell_order_mask,
        consumed_slice_count,
        state,
    };
    value.validate()?;
    if value.presence_bits() != presence {
        return Err(Error::InvalidExpectation);
    }
    Ok(value)
}

/// Exact owner cash disposition derived from the mutable V4 handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementDispositionV4 {
    buy_cash_handoff_atoms: Amount,
    consideration_debit_atoms: Amount,
    selected_fee_atoms: Amount,
    total_debit_atoms: Amount,
    credit_atoms: Amount,
    released_cash_atoms: Amount,
    residue_price_units: u128,
    position_cash_atoms: Amount,
    position_reserved_cash_atoms: Amount,
}

impl OwnerSettlementDispositionV4 {
    /// Exact Reservation cash ownership removed from Position reserved cash.
    pub const fn buy_cash_handoff_atoms(&self) -> Amount {
        self.buy_cash_handoff_atoms
    }

    /// Aggregate buyer consideration converted once with ceiling.
    pub const fn consideration_debit_atoms(&self) -> Amount {
        self.consideration_debit_atoms
    }

    /// Selected owner fee included in the exact handoff debit.
    pub const fn selected_fee_atoms(&self) -> Amount {
        self.selected_fee_atoms
    }

    /// Buyer consideration plus selected fee.
    pub const fn total_debit_atoms(&self) -> Amount {
        self.total_debit_atoms
    }

    /// Aggregate seller credit converted once with floor.
    pub const fn credit_atoms(&self) -> Amount {
        self.credit_atoms
    }

    /// Excess handed-off buyer cash returned to free Position cash.
    pub const fn released_cash_atoms(&self) -> Amount {
        self.released_cash_atoms
    }

    /// Exact non-fee terminal rounding residue in price units.
    pub const fn residue_price_units(&self) -> u128 {
        self.residue_price_units
    }

    /// Exact prospective total Position cash.
    pub const fn position_cash_atoms(&self) -> Amount {
        self.position_cash_atoms
    }

    /// Exact prospective Position reserved cash.
    pub const fn position_reserved_cash_atoms(&self) -> Amount {
        self.position_reserved_cash_atoms
    }
}

/// Recover the exact Position prestate consumed by one finalized V4 row.
///
/// Action 40 needs to authenticate the immediately preceding zero-fee
/// action-38 Replay transition after that transition has overwritten the live
/// Replay account. The finalized row is the semantic owner of the exact
/// handoff, buyer debit, seller credit, and rounding boundary, so it can
/// invert only the cash/reserved-cash portion of that transition. Every
/// identity, generation, rent, purpose, Egg balance, and child count remains
/// byte-identical to `position_poststate`.
pub fn recover_owner_cash_position_prestate_v4(
    finalized: OwnerSettlementAccumulatorV4,
    position_poststate: PositionAccountV3,
) -> Result<PositionAccountV3> {
    finalized.validate()?;
    position_poststate
        .validate()
        .map_err(|_| Error::InvalidAccount)?;
    if finalized.state != OwnerSettlementStateV4::Finalized {
        return Err(Error::Incomplete);
    }
    let expectation = finalized.expectation;
    let total_debit_atoms = owner_debit_atoms(
        expectation.expected_buy_price_units.value,
        expectation.price_scale,
        expectation.selected_fee_atoms,
    )?;
    let credit_atoms = owner_credit_atoms(
        expectation.expected_sell_price_units.value,
        expectation.price_scale,
    )?;
    let post = position_poststate.fields();
    let cash_atoms = post
        .cash_atoms
        .checked_add(total_debit_atoms)
        .ok_or(Error::ArithmeticOverflow)?
        .checked_sub(credit_atoms)
        .ok_or(Error::ArithmeticUnderflow)?;
    let reserved_cash_atoms = post
        .reserved_cash_atoms
        .checked_add(finalized.buy_cash_handoff_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    if cash_atoms < finalized.buy_cash_handoff_atoms
        || reserved_cash_atoms < finalized.buy_cash_handoff_atoms
    {
        return Err(Error::InvariantViolation);
    }
    let recovered = PositionAccountV3::new(PositionV3Fields {
        cash_atoms,
        reserved_cash_atoms,
        ..post
    })
    .map_err(|_| Error::InvalidAccount)?;
    let forward_cash = cash_atoms
        .checked_sub(finalized.buy_cash_handoff_atoms)
        .and_then(|value| {
            value.checked_add(
                finalized
                    .buy_cash_handoff_atoms
                    .checked_sub(total_debit_atoms)?,
            )
        })
        .and_then(|value| value.checked_add(credit_atoms))
        .ok_or(Error::ArithmeticOverflow)?;
    let forward_reserved = reserved_cash_atoms
        .checked_sub(finalized.buy_cash_handoff_atoms)
        .ok_or(Error::ArithmeticUnderflow)?;
    if forward_cash != post.cash_atoms || forward_reserved != post.reserved_cash_atoms {
        return Err(Error::InvariantViolation);
    }
    Ok(recovered)
}

/// Typed data identity of one exact finalized V4 owner row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct OwnerFinalizedRowDataIdV4([u8; 32]);

impl OwnerFinalizedRowDataIdV4 {
    /// Exact 32-byte finalized-row identity.
    pub const fn bytes(&self) -> [u8; 32] {
        self.0
    }
}

/// Hash boundary for one exact finalized V4 owner row.
pub trait OwnerFinalizedRowDataHashV4 {
    /// Compute SHA-256 over the domain followed by all 288 body bytes.
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32];
}

/// Derive the typed finalized-row identity from an exact state-two body.
pub fn derive_owner_finalized_row_data_id_v4<H: OwnerFinalizedRowDataHashV4>(
    finalized_body: &[u8; OWNER_SETTLEMENT_BODY_V4_BYTES],
    hash: &H,
) -> Result<OwnerFinalizedRowDataIdV4> {
    let row = decode_semantic_body(finalized_body)?;
    if row.state != OwnerSettlementStateV4::Finalized {
        return Err(Error::Incomplete);
    }
    let id = hash.sha256(OWNER_FINALIZED_ROW_DATA_ID_DOMAIN_V4, finalized_body);
    if id == [0; 32] {
        return Err(Error::InvalidIdentity);
    }
    Ok(OwnerFinalizedRowDataIdV4(id))
}

/// Immutable terminal projection from one finalized V4 row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementTerminalProjectionV4 {
    expectation: OwnerSettlementExpectationV4,
    finalized_body: [u8; OWNER_SETTLEMENT_BODY_V4_BYTES],
}

impl OwnerSettlementTerminalProjectionV4 {
    /// Immutable selected expectation.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV4 {
        self.expectation
    }

    /// Exact canonical finalized V4 body.
    pub const fn finalized_body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V4_BYTES] {
        &self.finalized_body
    }
}

/// Structural V4 PDA projection supplied by the General adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementPdaProjectionV4 {
    /// Program owning the V4 seed domain.
    pub program_id: [u8; 32],
    /// Derived owner-row address.
    pub address: [u8; 32],
    /// Parent Epoch PDA seed.
    pub epoch: [u8; 32],
    /// Final selected candidate seed.
    pub candidate: [u8; 32],
    /// Semantic owner seed.
    pub owner: [u8; 32],
    /// Canonical V4 PDA bump.
    pub bump: u8,
}

impl OwnerSettlementPdaProjectionV4 {
    fn validate(&self) -> Result<()> {
        if self.program_id == [0; 32]
            || self.address == [0; 32]
            || self.epoch == [0; 32]
            || self.candidate == [0; 32]
            || self.owner == [0; 32]
        {
            return Err(Error::InvalidAccount);
        }
        Ok(())
    }
}

/// Strict outer-account facts for one V4 owner row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementAccountViewV4<'a> {
    /// Presented V4 row address.
    pub address: [u8; 32],
    /// Presented program owner.
    pub program_owner: [u8; 32],
    /// Whether the account meta is writable.
    pub writable: bool,
    /// Authenticated General outer tag.
    pub outer_tag: u8,
    /// Authenticated General outer version.
    pub outer_version: u8,
    /// Stored V4 row PDA bump.
    pub stored_bump: u8,
    /// Current lamport balance.
    pub lamports: u64,
    /// Exact current rent minimum for the 292-byte envelope.
    pub rent_minimum: u64,
    /// Exact 288-byte semantic body.
    pub body: &'a [u8],
}

/// Structurally checked V4 row projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementAccountProjectionV4 {
    address: [u8; 32],
    program_id: [u8; 32],
    lamports: u64,
    rent_minimum: u64,
    accumulator: OwnerSettlementAccumulatorV4,
}

impl OwnerSettlementAccountProjectionV4 {
    /// Canonical owner-row PDA.
    pub const fn address(&self) -> [u8; 32] {
        self.address
    }

    /// Dragon's Clutch program identity.
    pub const fn program_id(&self) -> [u8; 32] {
        self.program_id
    }

    /// Current lamports.
    pub const fn lamports(&self) -> u64 {
        self.lamports
    }

    /// Current exact rent minimum.
    pub const fn rent_minimum(&self) -> u64 {
        self.rent_minimum
    }

    /// Exact decoded semantic accumulator.
    pub const fn accumulator(&self) -> OwnerSettlementAccumulatorV4 {
        self.accumulator
    }
}

/// Project an existing exact `0x81/4` owner row.
pub fn project_owner_settlement_account_v4(
    view: OwnerSettlementAccountViewV4<'_>,
    derived: OwnerSettlementPdaProjectionV4,
) -> Result<OwnerSettlementAccountProjectionV4> {
    derived.validate()?;
    if !view.writable
        || view.address != derived.address
        || view.program_owner != derived.program_id
        || view.stored_bump != derived.bump
        || view.lamports < view.rent_minimum
        || view.body.len() != OWNER_SETTLEMENT_BODY_V4_BYTES
    {
        return Err(Error::InvalidAccount);
    }
    let accumulator = OwnerSettlementAccumulatorV4::decode_body(
        view.outer_tag,
        view.outer_version,
        view.body,
    )?;
    let expectation = accumulator.expectation;
    if expectation.epoch != derived.epoch
        || expectation.candidate != derived.candidate
        || expectation.owner != derived.owner
    {
        return Err(Error::InvalidAccount);
    }
    Ok(OwnerSettlementAccountProjectionV4 {
        address: view.address,
        program_id: derived.program_id,
        lamports: view.lamports,
        rent_minimum: view.rent_minimum,
        accumulator,
    })
}

/// Counted SettlementRoot authority facts for one V4 owner-row creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct SettlementRootOwnerRowAuthorityV4 {
    /// Counted SettlementRoot account PDA.
    pub settlement_root_account: [u8; 32],
    /// Complete V4 expectation.
    pub expectation: OwnerSettlementExpectationV4,
    /// Zero-based owner-sorted row ordinal.
    pub row_ordinal: u16,
    /// Exact owner count.
    pub owner_count: u16,
    /// Present rent payer.
    pub rent_payer: [u8; 32],
    /// Sole eventual refund recipient.
    pub rent_refund_recipient: [u8; 32],
    /// Persisted rent ledger.
    pub rent_ledger: [u8; 32],
    /// Canonical prefund donation sink.
    pub donation_sink: [u8; 32],
}

impl SettlementRootOwnerRowAuthorityV4 {
    fn validate(&self) -> Result<()> {
        self.expectation.validate()?;
        if self.settlement_root_account == [0; 32]
            || self.owner_count == 0
            || self.row_ordinal >= self.owner_count
            || self.rent_payer == [0; 32]
            || self.rent_refund_recipient == [0; 32]
            || self.rent_ledger == [0; 32]
            || self.donation_sink == [0; 32]
            || self.rent_ledger == self.donation_sink
            || self.rent_ledger == self.settlement_root_account
            || self.rent_ledger == self.rent_payer
            || self.rent_ledger == self.rent_refund_recipient
            || self.donation_sink == self.settlement_root_account
            || self.donation_sink == self.rent_payer
            || self.donation_sink == self.rent_refund_recipient
        {
            return Err(Error::AuthorityUnavailable);
        }
        Ok(())
    }
}

/// Atomic rent-safe V4 owner-row creation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementCreatePlanV4 {
    address: [u8; 32],
    program_id: [u8; 32],
    bump: u8,
    payer_debit_lamports: u64,
    target_lamports_after: u64,
    refund_recipient: [u8; 32],
    rent_ledger: [u8; 32],
    payer_rent_principal_lamports: u64,
    prefunded_donation_lamports: u64,
    donation_sink: [u8; 32],
    body: [u8; OWNER_SETTLEMENT_BODY_V4_BYTES],
}

impl OwnerSettlementCreatePlanV4 {
    /// Account to allocate and assign.
    pub const fn address(&self) -> [u8; 32] {
        self.address
    }

    /// Program owner after assignment.
    pub const fn program_id(&self) -> [u8; 32] {
        self.program_id
    }

    /// Canonical stored bump.
    pub const fn bump(&self) -> u8 {
        self.bump
    }

    /// Present payer debit.
    pub const fn payer_debit_lamports(&self) -> u64 {
        self.payer_debit_lamports
    }

    /// Final target lamport balance.
    pub const fn target_lamports_after(&self) -> u64 {
        self.target_lamports_after
    }

    /// Sole eventual refund recipient.
    pub const fn refund_recipient(&self) -> [u8; 32] {
        self.refund_recipient
    }

    /// Persisted rent ledger.
    pub const fn rent_ledger(&self) -> [u8; 32] {
        self.rent_ledger
    }

    /// Maximum refundable payer principal.
    pub const fn payer_rent_principal_lamports(&self) -> u64 {
        self.payer_rent_principal_lamports
    }

    /// Unsolicited prefunding routed to the donation sink at closure.
    pub const fn prefunded_donation_lamports(&self) -> u64 {
        self.prefunded_donation_lamports
    }

    /// Canonical prefund donation sink.
    pub const fn donation_sink(&self) -> [u8; 32] {
        self.donation_sink
    }

    /// Exact pristine V4 semantic body.
    pub const fn body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V4_BYTES] {
        &self.body
    }
}

/// Prepare rent-safe creation of a pristine V4 owner row.
pub fn prepare_create_owner_settlement_account_v4(
    authority: SettlementRootOwnerRowAuthorityV4,
    derived: OwnerSettlementPdaProjectionV4,
    funding: OwnerSettlementCreateFundingV1,
) -> Result<OwnerSettlementCreatePlanV4> {
    authority.validate()?;
    derived.validate()?;
    if derived.epoch != authority.expectation.epoch
        || derived.candidate != authority.expectation.candidate
        || derived.owner != authority.expectation.owner
        || funding.payer == [0; 32]
        || funding.refund_recipient == [0; 32]
        || funding.payer != authority.rent_payer
        || funding.refund_recipient != authority.rent_refund_recipient
        || funding.payer == derived.address
        || funding.refund_recipient == derived.address
        || authority.rent_ledger == derived.address
        || authority.donation_sink == derived.address
        || funding.system_program_id == [0; 32]
        || funding.system_program_id == derived.program_id
        || funding.target_owner_before != funding.system_program_id
        || funding.target_data_len_before != 0
        || !funding.target_writable
        || funding.target_executable
        || funding.rent_minimum == 0
    {
        return Err(Error::InvalidAccount);
    }
    let payer_debit_lamports = funding
        .rent_minimum
        .saturating_sub(funding.target_lamports_before);
    if funding.payer_lamports < payer_debit_lamports {
        return Err(Error::InsufficientCash);
    }
    let row = OwnerSettlementAccumulatorV4::new(authority.expectation)?;
    Ok(OwnerSettlementCreatePlanV4 {
        address: derived.address,
        program_id: derived.program_id,
        bump: derived.bump,
        payer_debit_lamports,
        target_lamports_after: funding
            .target_lamports_before
            .checked_add(payer_debit_lamports)
            .ok_or(Error::ArithmeticOverflow)?,
        refund_recipient: funding.refund_recipient,
        rent_ledger: authority.rent_ledger,
        payer_rent_principal_lamports: payer_debit_lamports,
        prefunded_donation_lamports: funding.target_lamports_before,
        donation_sink: authority.donation_sink,
        body: row.encode_body()?,
    })
}

/// Non-authorizing exact row/receipt/Reservation accounting projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementReceiptAccountingProjectionV4Row {
    owner_settlement_account: [u8; 32],
    owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V4_BYTES],
    receipt: [u8; 32],
    receipt_data_id: SettlementReceiptDataIdV4,
    receipt_accounting_id: [u8; 32],
    receipt_accounted_end_mask: u8,
    reservation_handoff: Option<AuthenticatedReservationHandoffV3>,
}

impl OwnerSettlementReceiptAccountingProjectionV4Row {
    /// Owner row to compare-and-write.
    pub const fn owner_settlement_account(&self) -> [u8; 32] {
        self.owner_settlement_account
    }

    /// Exact canonical V4 row successor body.
    pub const fn owner_settlement_body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V4_BYTES] {
        &self.owner_settlement_body
    }

    /// Receipt account to latch.
    pub const fn receipt(&self) -> [u8; 32] {
        self.receipt
    }

    /// Exact typed receipt prestate identity.
    pub const fn receipt_data_id(&self) -> SettlementReceiptDataIdV4 {
        self.receipt_data_id
    }

    /// Stable accounting-only transition identity.
    pub const fn receipt_accounting_id(&self) -> [u8; 32] {
        self.receipt_accounting_id
    }

    /// Exact next independent accounting mask.
    pub const fn receipt_accounted_end_mask(&self) -> u8 {
        self.receipt_accounted_end_mask
    }

    /// Exact authenticated Reservation handoff, present only for terminal buy.
    pub const fn reservation_handoff(&self) -> Option<AuthenticatedReservationHandoffV3> {
        self.reservation_handoff
    }
}

/// Project one exact Receipt V4 end onto the canonical V4 owner row.
pub fn project_owner_receipt_end_to_owner_v4(
    account: OwnerSettlementAccountProjectionV4,
    receipt: AuthenticatedSettlementReceiptEndV4,
) -> Result<OwnerSettlementReceiptAccountingProjectionV4Row> {
    receipt.validate()?;
    let mut next = account.accumulator;
    next.consume(&receipt)?;
    Ok(OwnerSettlementReceiptAccountingProjectionV4Row {
        owner_settlement_account: account.address,
        owner_settlement_body: next.encode_body()?,
        receipt: receipt.receipt,
        receipt_data_id: receipt.receipt_data_id,
        receipt_accounting_id: receipt.receipt_accounting_id,
        receipt_accounted_end_mask: receipt.accounted_end_mask | receipt.side_mask(),
        reservation_handoff: receipt.reservation_handoff,
    })
}

/// Structural facts rederived from one exact accounted, undelivered merge Receipt V4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerMergeDeliveryEvidenceV4 {
    /// Canonical Receipt V4 account.
    pub receipt: [u8; 32],
    /// Exact mutable receipt-prestate identity.
    pub receipt_data_id: SettlementReceiptDataIdV4,
    /// Stable Receipt-PDA-derived delivery transition identity.
    pub delivery_transition_id: [u8; 32],
    /// General MarketRuntime identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Semantic seller owner.
    pub owner: [u8; 32],
    /// Canonical seller order identity.
    pub order_id: [u8; 32],
    /// Zero-based selected slice index.
    pub slice_index: u16,
    /// Exactly `slice_index + 1`.
    pub sequence: u64,
}

impl OwnerMergeDeliveryEvidenceV4 {
    fn validate(&self) -> Result<()> {
        for identity in [
            self.receipt,
            self.receipt_data_id.bytes(),
            self.delivery_transition_id,
            self.market,
            self.epoch,
            self.candidate,
            self.owner,
            self.order_id,
        ] {
            if identity == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if self.sequence != u64::from(self.slice_index) + 1
            || self.receipt_data_id.bytes() == self.delivery_transition_id
        {
            return Err(Error::InvalidOrder);
        }
        Ok(())
    }
}

/// Non-authorizing V4 owner-row successor for one atomic action-37 delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerMergeDeliveryProjectionV4 {
    owner_settlement_account: [u8; 32],
    owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V4_BYTES],
    receipt: [u8; 32],
    receipt_data_id: SettlementReceiptDataIdV4,
    delivery_transition_id: [u8; 32],
    delivered_count: u16,
}

impl OwnerMergeDeliveryProjectionV4 {
    /// Canonical V4 owner row to compare-and-write.
    pub const fn owner_settlement_account(&self) -> [u8; 32] {
        self.owner_settlement_account
    }
    /// Exact V4 owner-row successor body.
    pub const fn owner_settlement_body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V4_BYTES] {
        &self.owner_settlement_body
    }
    /// Canonical merge Receipt V4.
    pub const fn receipt(&self) -> [u8; 32] {
        self.receipt
    }
    /// Exact receipt prestate data identity.
    pub const fn receipt_data_id(&self) -> SettlementReceiptDataIdV4 {
        self.receipt_data_id
    }
    /// Exact action-37 delivery transition identity.
    pub const fn delivery_transition_id(&self) -> [u8; 32] {
        self.delivery_transition_id
    }
    /// Monotone delivered merge-end count after this transition.
    pub const fn delivered_count(&self) -> u16 {
        self.delivered_count
    }
}

/// Advance the exact owner-scoped merge-delivery latch once.
pub fn project_owner_merge_delivery_v4(
    account: OwnerSettlementAccountProjectionV4,
    evidence: OwnerMergeDeliveryEvidenceV4,
) -> Result<OwnerMergeDeliveryProjectionV4> {
    evidence.validate()?;
    let expectation = account.accumulator.expectation();
    if evidence.market != expectation.market()
        || evidence.epoch != expectation.epoch()
        || evidence.candidate != expectation.candidate()
        || evidence.owner != expectation.owner()
        || expectation.expected_merge_delivery_count() == 0
    {
        return Err(Error::AuthorityUnavailable);
    }
    let mut next = account.accumulator;
    next.record_merge_delivery()?;
    Ok(OwnerMergeDeliveryProjectionV4 {
        owner_settlement_account: account.address,
        owner_settlement_body: next.encode_body()?,
        receipt: evidence.receipt,
        receipt_data_id: evidence.receipt_data_id,
        delivery_transition_id: evidence.delivery_transition_id,
        delivered_count: next.merge_delivered_count()?,
    })
}

/// Structural V4 row, Position, and settlement-pot realization plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerCashRealizationPlanV4 {
    owner_settlement_account: [u8; 32],
    expectation: OwnerSettlementExpectationV4,
    owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V4_BYTES],
    finalized_row_data_id: OwnerFinalizedRowDataIdV4,
    position: PositionSettlementPoststateV3,
    settlement_cash_pot: SettlementCashPotV1,
    disposition: OwnerSettlementDispositionV4,
}

/// Outer-version-neutral V4 semantic cash realization.
///
/// This plan owns the one arithmetic transition shared by historical V4 and
/// the rent-owned V5 envelope. It deliberately derives no outer-row data ID;
/// the versioned General envelope must hash its complete poststate separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerCashRealizationSemanticPlanV4 {
    owner_settlement_account: [u8; 32],
    expectation: OwnerSettlementExpectationV4,
    owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V4_BYTES],
    position: PositionSettlementPoststateV3,
    settlement_cash_pot: SettlementCashPotV1,
    disposition: OwnerSettlementDispositionV4,
}

impl OwnerCashRealizationSemanticPlanV4 {
    /// Owner row whose versioned envelope must be compare-and-written.
    pub const fn owner_settlement_account(&self) -> [u8; 32] {
        self.owner_settlement_account
    }

    /// Immutable selected expectation.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV4 {
        self.expectation
    }

    /// Exact finalized 288-byte semantic body.
    pub const fn owner_settlement_body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V4_BYTES] {
        &self.owner_settlement_body
    }

    /// Exact canonical Position V3 successor.
    pub const fn position(&self) -> PositionSettlementPoststateV3 {
        self.position
    }

    /// Exact candidate-wide settlement-pot successor.
    pub const fn settlement_cash_pot(&self) -> SettlementCashPotV1 {
        self.settlement_cash_pot
    }

    /// Exact handoff-derived Floor/Ceil disposition.
    pub const fn disposition(&self) -> OwnerSettlementDispositionV4 {
        self.disposition
    }
}

impl OwnerCashRealizationPlanV4 {
    /// V4 owner row to compare-and-write.
    pub const fn owner_settlement_account(&self) -> [u8; 32] {
        self.owner_settlement_account
    }

    /// Immutable selected expectation.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV4 {
        self.expectation
    }

    /// Exact finalized V4 owner-row body.
    pub const fn owner_settlement_body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V4_BYTES] {
        &self.owner_settlement_body
    }

    /// Typed exact finalized-row data identity.
    pub const fn finalized_row_data_id(&self) -> OwnerFinalizedRowDataIdV4 {
        self.finalized_row_data_id
    }

    /// Exact canonical Position V3 successor.
    pub const fn position(&self) -> PositionSettlementPoststateV3 {
        self.position
    }

    /// Exact candidate-wide settlement-pot successor.
    pub const fn settlement_cash_pot(&self) -> SettlementCashPotV1 {
        self.settlement_cash_pot
    }

    /// Exact handoff-derived Floor/Ceil disposition.
    pub const fn disposition(&self) -> OwnerSettlementDispositionV4 {
        self.disposition
    }
}

/// Realize one accounting-complete V4 row without external funding summaries.
pub fn prepare_realize_owner_cash_v4<H: OwnerFinalizedRowDataHashV4>(
    account: OwnerSettlementAccountProjectionV4,
    position: AuthenticatedPositionV3,
    pot: SettlementCashPotV1,
    hash: &H,
) -> Result<OwnerCashRealizationPlanV4> {
    let semantic = prepare_realize_owner_cash_semantic_v4(
        account.address,
        account.accumulator,
        position,
        pot,
    )?;
    let finalized_row_data_id =
        derive_owner_finalized_row_data_id_v4(semantic.owner_settlement_body(), hash)?;
    Ok(OwnerCashRealizationPlanV4 {
        owner_settlement_account: semantic.owner_settlement_account,
        expectation: semantic.expectation,
        owner_settlement_body: semantic.owner_settlement_body,
        finalized_row_data_id,
        position: semantic.position,
        settlement_cash_pot: semantic.settlement_cash_pot,
        disposition: semantic.disposition,
    })
}

/// Realize one exact V4 semantic row independent of its General outer version.
pub fn prepare_realize_owner_cash_semantic_v4(
    owner_settlement_account: [u8; 32],
    accumulator: OwnerSettlementAccumulatorV4,
    position: AuthenticatedPositionV3,
    pot: SettlementCashPotV1,
) -> Result<OwnerCashRealizationSemanticPlanV4> {
    pot.validate()?;
    position.validate_writable()?;
    let position_prestate = position;
    let expected = accumulator.expectation;
    let position_fields = position.semantic.fields();
    if pot.state != 0
        || position.general_market_runtime != expected.market
        || position_fields.owner.bytes() != expected.owner
        || pot.expectation.market != expected.market
        || pot.expectation.epoch != expected.epoch
        || pot.expectation.candidate != expected.candidate
        || pot.expectation.owner_order_set_digest != expected.owner_order_set_digest
        || owner_settlement_account == [0; 32]
        || owner_settlement_account == position.account
    {
        return Err(Error::AuthorityUnavailable);
    }
    let mut next_row = accumulator;
    let disposition = next_row.finalize(
        position_fields.cash_atoms,
        position_fields.reserved_cash_atoms,
    )?;
    let available_consideration_atoms = pot
        .available_consideration_atoms
        .checked_add(disposition.consideration_debit_atoms)
        .ok_or(Error::ArithmeticOverflow)?
        .checked_sub(disposition.credit_atoms)
        .ok_or(Error::SettlementLiquidityUnavailable)?;
    let mut next_pot = pot;
    next_pot.available_consideration_atoms = available_consideration_atoms;
    next_pot.collected_fee_atoms = next_pot
        .collected_fee_atoms
        .checked_add(disposition.selected_fee_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    next_pot.realized_rounding_price_units = next_pot
        .realized_rounding_price_units
        .checked_add(disposition.residue_price_units)
        .ok_or(Error::ArithmeticOverflow)?;
    next_pot.finalized_owner_count = next_pot
        .finalized_owner_count
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    if next_pot.finalized_owner_count == next_pot.expectation.owner_count {
        next_pot.state = 1;
    }
    next_pot.validate()?;
    let position = position_prestate.settlement_poststate(
        disposition.position_cash_atoms,
        disposition.position_reserved_cash_atoms,
        position_fields.native_eggs,
    )?;
    position.validate_successor_of(
        position_prestate,
        disposition.position_cash_atoms,
        disposition.position_reserved_cash_atoms,
        position_fields.native_eggs,
    )?;
    let owner_settlement_body = next_row.encode_body()?;
    Ok(OwnerCashRealizationSemanticPlanV4 {
        owner_settlement_account,
        expectation: expected,
        owner_settlement_body,
        position,
        settlement_cash_pot: next_pot,
        disposition,
    })
}

fn order_bit(order_index: u8) -> Result<u64> {
    if usize::from(order_index) >= MAX_ORDERS {
        return Err(Error::InvalidOrder);
    }
    1u64
        .checked_shl(u32::from(order_index))
        .ok_or(Error::InvalidOrder)
}

fn put<const N: usize>(output: &mut [u8; N], cursor: &mut usize, bytes: &[u8]) -> Result<()> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(Error::ArithmeticOverflow)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::InvalidExpectation)?
        .copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

fn take<'a>(input: &'a [u8], cursor: &mut usize, width: usize) -> Result<&'a [u8]> {
    let end = cursor.checked_add(width).ok_or(Error::ArithmeticOverflow)?;
    let value = input.get(*cursor..end).ok_or(Error::InvalidExpectation)?;
    *cursor = end;
    Ok(value)
}

fn read_key(input: &[u8], cursor: &mut usize) -> Result<[u8; 32]> {
    let mut value = [0u8; 32];
    value.copy_from_slice(take(input, cursor, 32)?);
    Ok(value)
}

fn read_u8(input: &[u8], cursor: &mut usize) -> Result<u8> {
    Ok(take(input, cursor, 1)?[0])
}

fn read_u16(input: &[u8], cursor: &mut usize) -> Result<u16> {
    let mut value = [0u8; 2];
    value.copy_from_slice(take(input, cursor, 2)?);
    Ok(u16::from_le_bytes(value))
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut value = [0u8; 8];
    value.copy_from_slice(take(input, cursor, 8)?);
    Ok(u64::from_le_bytes(value))
}

fn read_u128(input: &[u8], cursor: &mut usize) -> Result<u128> {
    let mut value = [0u8; 16];
    value.copy_from_slice(take(input, cursor, 16)?);
    Ok(u128::from_le_bytes(value))
}

const _: () = assert!(OWNER_SETTLEMENT_BODY_V4_BYTES == 288);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{derive_settlement_receipt_data_id_v4, SettlementReceiptDataHashV4,
        SettlementReceiptRouteV4, SETTLEMENT_RECEIPT_BODY_V4_BYTES};

    #[derive(Clone, Copy, Debug)]
    struct PrefixHash;

    impl SettlementReceiptDataHashV4 for PrefixHash {
        fn sha256(&self, domain: &[u8], transcript: &[u8]) -> [u8; 32] {
            let mut value = [17u8; 32];
            value[0] = domain[0];
            value[1] = transcript[0];
            value
        }
    }

    fn merge_expectation() -> OwnerSettlementExpectationV4 {
        let mut orders = [VerifiedSettlementOrderV4 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::ABSENT,
            slice_count: 0,
            merge_delivery_count: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV4 {
            owner: [6; 32],
            order_index: 4,
            side: SettlementSideV1::Sell,
            consideration_price_units: PresentConsiderationV2::new(100),
            slice_count: 2,
            merge_delivery_count: 2,
        };
        build_owner_settlement_expectation_basis_book_v4(
            [1; 32], [2; 32], [3; 32], [4; 32], 100, &orders, 1,
        )
        .unwrap()
        .row(0)
        .unwrap()
        .with_selected_fee(SelectedOwnerFeeV1 { owner: [6; 32], fee_atoms: 0 })
        .unwrap()
    }

    fn direct_sell_expectation() -> OwnerSettlementExpectationV4 {
        let mut orders = [VerifiedSettlementOrderV4 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::ABSENT,
            slice_count: 0,
            merge_delivery_count: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV4 {
            owner: [6; 32],
            order_index: 4,
            side: SettlementSideV1::Sell,
            consideration_price_units: PresentConsiderationV2::new(100),
            slice_count: 1,
            merge_delivery_count: 0,
        };
        build_owner_settlement_expectation_basis_book_v4(
            [1; 32], [2; 32], [3; 32], [4; 32], 100, &orders, 1,
        )
        .unwrap()
        .row(0)
        .unwrap()
        .with_selected_fee(SelectedOwnerFeeV1 { owner: [6; 32], fee_atoms: 0 })
        .unwrap()
    }

    fn receipt(slice_index: u16, completes_order: bool) -> AuthenticatedSettlementReceiptEndV4 {
        let mut body = [0u8; SETTLEMENT_RECEIPT_BODY_V4_BYTES];
        body[..2].copy_from_slice(&[0x0f, 4]);
        AuthenticatedSettlementReceiptEndV4 {
            receipt: [11; 32],
            receipt_data_id: derive_settlement_receipt_data_id_v4([11; 32], &body, &PrefixHash)
                .unwrap(),
            receipt_accounting_id: [13; 32],
            market: [1; 32],
            epoch: [2; 32],
            candidate: [3; 32],
            owner_order_set_digest: [4; 32],
            owner: [6; 32],
            order_id: [7; 32],
            order_index: 4,
            side: SettlementSideV1::Sell,
            route: SettlementReceiptRouteV4::SellToMerge,
            consideration_price_units: PresentConsiderationV2::new(50),
            completes_order,
            slice_index,
            sequence: u64::from(slice_index) + 1,
            accounted_end_mask: 0,
            expected_end_mask: SELL_END_MASK,
            reservation_handoff: None,
        }
    }

    fn direct_sell_receipt() -> AuthenticatedSettlementReceiptEndV4 {
        let mut value = receipt(0, true);
        value.route = SettlementReceiptRouteV4::Direct;
        value.consideration_price_units = PresentConsiderationV2::new(100);
        value.expected_end_mask = BUY_END_MASK | SELL_END_MASK;
        value
    }

    #[test]
    fn merge_cash_refuses_until_every_delivery_is_counted() {
        let mut row = OwnerSettlementAccumulatorV4::new(merge_expectation()).unwrap();
        row.consume(&receipt(0, false)).unwrap();
        row.consume(&receipt(1, true)).unwrap();
        assert_eq!(row.state(), OwnerSettlementStateV4::AccountingComplete);
        assert_eq!(row.progress_count(), 0);
        assert_eq!(row.finalize(0, 0), Err(Error::Incomplete));
        row.record_merge_delivery().unwrap();
        assert_eq!(row.finalize(0, 0), Err(Error::Incomplete));
        row.record_merge_delivery().unwrap();
        assert!(row.finalize(0, 0).is_ok());
        assert_eq!(row.state(), OwnerSettlementStateV4::Finalized);
        assert_eq!(row.record_merge_delivery(), Err(Error::Incomplete));
    }

    #[test]
    fn duplicate_merge_delivery_cannot_advance_past_the_immutable_count() {
        let mut expectation = merge_expectation();
        expectation.expected_merge_delivery_count = 1;
        let mut row = OwnerSettlementAccumulatorV4::new(expectation).unwrap();
        row.consume(&receipt(0, false)).unwrap();
        row.consume(&receipt(1, true)).unwrap();
        row.record_merge_delivery().unwrap();
        assert_eq!(row.record_merge_delivery(), Err(Error::Incomplete));
    }

    #[test]
    fn zero_merge_owner_is_immediately_eligible_after_accounting() {
        let mut row = OwnerSettlementAccumulatorV4::new(direct_sell_expectation()).unwrap();
        row.consume(&direct_sell_receipt()).unwrap();
        assert_eq!(row.merge_delivered_count(), Ok(0));
        assert!(row.finalize(0, 0).is_ok());
    }

    #[test]
    fn builder_refuses_buy_merge_counts_and_counts_above_end_total() {
        let mut orders = [VerifiedSettlementOrderV4 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::ABSENT,
            slice_count: 0,
            merge_delivery_count: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV4 {
            owner: [6; 32],
            order_index: 4,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::new(1),
            slice_count: 1,
            merge_delivery_count: 1,
        };
        assert_eq!(
            build_owner_settlement_expectation_basis_book_v4(
                [1; 32], [2; 32], [3; 32], [4; 32], 100, &orders, 1,
            ),
            Err(Error::InvalidOrder)
        );
        orders[0].side = SettlementSideV1::Sell;
        orders[0].merge_delivery_count = 2;
        assert_eq!(
            build_owner_settlement_expectation_basis_book_v4(
                [1; 32], [2; 32], [3; 32], [4; 32], 100, &orders, 1,
            ),
            Err(Error::InvalidOrder)
        );
    }

    #[test]
    fn v4_refuses_v3_outer_and_hostile_merge_count() {
        let row = OwnerSettlementAccumulatorV4::new(merge_expectation()).unwrap();
        let mut body = row.encode_body().unwrap();
        assert_eq!(
            OwnerSettlementAccumulatorV4::decode_body(0x81, 3, &body),
            Err(Error::InvalidAccount)
        );
        body[286..288].copy_from_slice(&3u16.to_le_bytes());
        assert_eq!(
            OwnerSettlementAccumulatorV4::decode_body(0x81, 4, &body),
            Err(Error::InvalidExpectation)
        );
    }
}
