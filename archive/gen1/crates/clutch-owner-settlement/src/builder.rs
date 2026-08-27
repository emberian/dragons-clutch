//! Canonical projection from authenticated selected-order rows to owner rows.

use crate::{
    owner_credit_atoms, owner_debit_atoms, owner_rounding_residue_price_units, Amount, Error,
    OwnerSettlementExpectationV1, Result, SettlementSideV1, MAX_ORDERS,
};

/// One filled order after exact selected-candidate/order-set authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VerifiedSettlementOrderV1 {
    /// Semantic Position owner.
    pub owner: [u8; 32],
    /// Canonical selected order index.
    pub order_index: u8,
    /// Payer or payee side.
    pub side: SettlementSideV1,
    /// Aggregate exact value of this filled order in price units.
    pub consideration_price_units: u128,
    /// Exact receipt ends that consume this order.
    pub slice_count: u16,
    /// Exact cash reservation envelope; zero for sell orders.
    pub reserved_cash_atoms: Amount,
}

/// Selected owner-scoped fee assessment joined from the fee runtime contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct SelectedOwnerFeeV1 {
    /// Participating owner.
    pub owner: [u8; 32],
    /// Whole collateral fee atoms already selected for this owner.
    pub fee_atoms: Amount,
}

impl SelectedOwnerFeeV1 {
    /// Canonical unused fixed-capacity row.
    pub const EMPTY: Self = Self {
        owner: [0; 32],
        fee_atoms: 0,
    };
}

/// Candidate-summary totals that the owner projection must reproduce exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct CandidateSettlementTotalsV1 {
    /// Exact participating owner count.
    pub owner_count: u16,
    /// Sum of owner buy considerations in price units.
    pub buy_price_units: u128,
    /// Sum of owner sell credits in price units.
    pub sell_price_units: u128,
    /// Sum of selected owner fee atoms.
    pub selected_fee_atoms: u128,
    /// Sum of non-fee owner terminal-rounding slack.
    pub rounding_pot_price_units: u128,
    /// Sum of expected receipt ends across owners.
    pub owner_slice_end_count: u16,
}

/// Canonically owner-sorted settlement expectations for one candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementBookV1 {
    /// Active owner prefix, lexicographically sorted by owner key.
    pub rows: [OwnerSettlementExpectationV1; MAX_ORDERS],
    /// Active row count.
    pub owner_count: u16,
    /// Whole owner debits including selected fee atoms.
    pub debit_atoms: u128,
    /// Whole owner credits.
    pub credit_atoms: u128,
    /// Exact non-fee terminal-rounding slack.
    pub rounding_pot_price_units: u128,
}

/// Recompute every owner expectation from selected, authenticated order rows.
///
/// `orders[..order_len]` must be the complete filled-order set. `fees[..fee_len]`
/// must contain exactly one row for every participating owner, including an
/// explicit zero row. This makes fee absence different from a missing join.
#[allow(clippy::too_many_arguments)]
pub fn build_owner_settlement_book_v1(
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner_order_set_digest: [u8; 32],
    price_scale: Amount,
    orders: &[VerifiedSettlementOrderV1; MAX_ORDERS],
    order_len: u8,
    fees: &[SelectedOwnerFeeV1; MAX_ORDERS],
    fee_len: u8,
    expected: CandidateSettlementTotalsV1,
) -> Result<OwnerSettlementBookV1> {
    if market == [0; 32]
        || epoch == [0; 32]
        || candidate == [0; 32]
        || owner_order_set_digest == [0; 32]
        || price_scale == 0
        || order_len == 0
        || usize::from(order_len) > MAX_ORDERS
        || usize::from(fee_len) > MAX_ORDERS
    {
        return Err(Error::InvalidExpectation);
    }
    let mut rows = [OwnerSettlementExpectationV1::EMPTY; MAX_ORDERS];
    let mut owner_count = 0_usize;
    let mut seen_order_mask = 0_u64;
    let mut order_at = 0_usize;
    while order_at < usize::from(order_len) {
        let order = orders[order_at];
        if order.owner == [0; 32]
            || order.consideration_price_units == 0
            || order.slice_count == 0
            || usize::from(order.order_index) >= MAX_ORDERS
            || (order.side == SettlementSideV1::Sell && order.reserved_cash_atoms != 0)
        {
            return Err(Error::InvalidOrder);
        }
        let bit = 1_u64
            .checked_shl(u32::from(order.order_index))
            .ok_or(Error::InvalidOrder)?;
        if seen_order_mask & bit != 0 {
            return Err(Error::InvalidOrder);
        }
        seen_order_mask |= bit;
        let mut slot = 0_usize;
        while slot < owner_count && rows[slot].owner != order.owner {
            slot += 1;
        }
        if slot == owner_count {
            if owner_count >= MAX_ORDERS {
                return Err(Error::ArithmeticOverflow);
            }
            rows[slot] = OwnerSettlementExpectationV1 {
                market,
                epoch,
                candidate,
                owner: order.owner,
                owner_order_set_digest,
                price_scale,
                expected_buy_order_mask: 0,
                expected_sell_order_mask: 0,
                expected_slice_count: 0,
                expected_buy_price_units: 0,
                expected_sell_price_units: 0,
                selected_fee_atoms: 0,
                reserved_cash_atoms: 0,
            };
            owner_count += 1;
        }
        rows[slot].expected_slice_count = rows[slot]
            .expected_slice_count
            .checked_add(order.slice_count)
            .ok_or(Error::ArithmeticOverflow)?;
        match order.side {
            SettlementSideV1::Buy => {
                rows[slot].expected_buy_order_mask |= bit;
                rows[slot].expected_buy_price_units = rows[slot]
                    .expected_buy_price_units
                    .checked_add(order.consideration_price_units)
                    .ok_or(Error::ArithmeticOverflow)?;
                rows[slot].reserved_cash_atoms = rows[slot]
                    .reserved_cash_atoms
                    .checked_add(order.reserved_cash_atoms)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            SettlementSideV1::Sell => {
                rows[slot].expected_sell_order_mask |= bit;
                rows[slot].expected_sell_price_units = rows[slot]
                    .expected_sell_price_units
                    .checked_add(order.consideration_price_units)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }
        order_at += 1;
    }
    if usize::from(fee_len) != owner_count {
        return Err(Error::InvalidExpectation);
    }
    let mut fee_seen_mask = 0_u64;
    let mut fee_at = 0_usize;
    while fee_at < usize::from(fee_len) {
        let fee = fees[fee_at];
        if fee.owner == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        let mut slot = 0_usize;
        while slot < owner_count && rows[slot].owner != fee.owner {
            slot += 1;
        }
        if slot == owner_count {
            return Err(Error::InvalidIdentity);
        }
        let bit = 1_u64
            .checked_shl(u32::try_from(slot).map_err(|_| Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)?;
        if fee_seen_mask & bit != 0 {
            return Err(Error::InvalidIdentity);
        }
        fee_seen_mask |= bit;
        rows[slot].selected_fee_atoms = fee.fee_atoms;
        fee_at += 1;
    }

    // Canonicalize account creation and later page order independently of the
    // order rows' first-occurrence order.
    let mut at = 1_usize;
    while at < owner_count {
        let value = rows[at];
        let mut insert = at;
        while insert > 0 && value.owner < rows[insert - 1].owner {
            rows[insert] = rows[insert - 1];
            insert -= 1;
        }
        rows[insert] = value;
        at += 1;
    }

    let mut buy_price_units = 0_u128;
    let mut sell_price_units = 0_u128;
    let mut selected_fee_atoms = 0_u128;
    let mut rounding_pot_price_units = 0_u128;
    let mut owner_slice_end_count = 0_u16;
    let mut debit_atoms = 0_u128;
    let mut credit_atoms = 0_u128;
    let mut owner_at = 0_usize;
    while owner_at < owner_count {
        rows[owner_at].validate()?;
        buy_price_units = buy_price_units
            .checked_add(rows[owner_at].expected_buy_price_units)
            .ok_or(Error::ArithmeticOverflow)?;
        sell_price_units = sell_price_units
            .checked_add(rows[owner_at].expected_sell_price_units)
            .ok_or(Error::ArithmeticOverflow)?;
        selected_fee_atoms = selected_fee_atoms
            .checked_add(u128::from(rows[owner_at].selected_fee_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        owner_slice_end_count = owner_slice_end_count
            .checked_add(rows[owner_at].expected_slice_count)
            .ok_or(Error::ArithmeticOverflow)?;
        rounding_pot_price_units = rounding_pot_price_units
            .checked_add(owner_rounding_residue_price_units(
                rows[owner_at].expected_buy_price_units,
                rows[owner_at].expected_sell_price_units,
                price_scale,
            )?)
            .ok_or(Error::ArithmeticOverflow)?;
        debit_atoms = debit_atoms
            .checked_add(u128::from(owner_debit_atoms(
                rows[owner_at].expected_buy_price_units,
                price_scale,
                rows[owner_at].selected_fee_atoms,
            )?))
            .ok_or(Error::ArithmeticOverflow)?;
        credit_atoms = credit_atoms
            .checked_add(u128::from(owner_credit_atoms(
                rows[owner_at].expected_sell_price_units,
                price_scale,
            )?))
            .ok_or(Error::ArithmeticOverflow)?;
        owner_at += 1;
    }
    let actual = CandidateSettlementTotalsV1 {
        owner_count: u16::try_from(owner_count).map_err(|_| Error::ArithmeticOverflow)?,
        buy_price_units,
        sell_price_units,
        selected_fee_atoms,
        rounding_pot_price_units,
        owner_slice_end_count,
    };
    if actual != expected {
        return Err(Error::InvariantViolation);
    }
    Ok(OwnerSettlementBookV1 {
        rows,
        owner_count: actual.owner_count,
        debit_atoms,
        credit_atoms,
        rounding_pot_price_units,
    })
}
