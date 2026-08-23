#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Exact owner-aggregated cash realization for General V2 settlement.
//!
//! Candidate verification supplies one immutable expectation per owner.
//! Receipt consumption accumulates exact price units across all of that
//! owner's filled orders. Collateral conversion happens once, only after every
//! expected order and slice has completed. This makes the runtime boundary
//! identical to the relation's owner-level `TerminalOwnerFloor` arithmetic.

/// Maximum orders in one frozen General book.
pub const MAX_ORDERS: usize = 64;
/// Exact persisted owner-settlement semantic body width.
///
/// General V2's central account registry owns the eventual outer tag/version;
/// this crate owns only the body and its canonical zero padding.
pub const OWNER_SETTLEMENT_BODY_V1_BYTES: usize = 288;

/// Exact atomic collateral quantity.
pub type Amount = u64;

/// Deterministic refusal from owner settlement staging.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// A key or digest is zero or aliases an independent semantic role.
    InvalidIdentity,
    /// Price scale, order masks, slice count, or expected totals are invalid.
    InvalidExpectation,
    /// An order index or side is inconsistent with the frozen masks.
    InvalidOrder,
    /// The fragment carries no exact price-unit value.
    ZeroFragment,
    /// A checked integer operation overflowed.
    ArithmeticOverflow,
    /// A checked integer operation underflowed.
    ArithmeticUnderflow,
    /// A completed order was completed again.
    DuplicateCompletion,
    /// More fragments arrived than the frozen owner row admits.
    TooManyFragments,
    /// Expected orders/fragments/totals have not all arrived.
    Incomplete,
    /// Authenticated Position or reservation cash cannot fund the result.
    InsufficientCash,
    /// A finalized or closed owner row was asked to mutate.
    Terminal,
    /// Prospective state does not close the exact conservation equations.
    InvariantViolation,
}

/// Result alias for owner settlement.
pub type Result<T> = core::result::Result<T, Error>;

/// Buy/payer or sell/payee side of one authenticated order fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementSideV1 {
    /// Payer side, converted once with ceiling.
    Buy = 0,
    /// Payee side, converted once with floor.
    Sell = 1,
}

/// Immutable verifier-owned expectation for one participating owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementExpectationV1 {
    /// Canonical General V2 Market-runtime identity.
    pub market: [u8; 32],
    /// Canonical counted Epoch identity.
    pub epoch: [u8; 32],
    /// Selected candidate identity.
    pub candidate: [u8; 32],
    /// Semantic Position owner.
    pub owner: [u8; 32],
    /// Digest of the exact ordered owner/order membership rows.
    pub owner_order_set_digest: [u8; 32],
    /// Exact collateral price scale.
    pub price_scale: Amount,
    /// Filled buy-order indices inside the candidate's canonical order set.
    pub expected_buy_order_mask: u64,
    /// Filled sell-order indices inside the candidate's canonical order set.
    pub expected_sell_order_mask: u64,
    /// Exact receipt fragments this owner must consume.
    pub expected_slice_count: u16,
    /// Aggregate consideration this owner owes before selected fee atoms.
    pub expected_buy_price_units: u128,
    /// Aggregate consideration this owner earns.
    pub expected_sell_price_units: u128,
    /// Already-selected, owner-scoped fee in whole collateral atoms.
    pub selected_fee_atoms: Amount,
    /// Exact cash atoms encumbered across this owner's buy reservations.
    pub reserved_cash_atoms: Amount,
}

impl OwnerSettlementExpectationV1 {
    /// Validate identity, disjoint masks, funding shape, and nonempty work.
    pub fn validate(&self) -> Result<()> {
        let keys = [self.market, self.epoch, self.candidate, self.owner];
        let mut left = 0_usize;
        while left < keys.len() {
            if keys[left] == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
            let mut right = left + 1;
            while right < keys.len() {
                if keys[left] == keys[right] {
                    return Err(Error::InvalidIdentity);
                }
                right += 1;
            }
            left += 1;
        }
        if self.owner_order_set_digest == [0; 32]
            || self.price_scale == 0
            || self.expected_slice_count == 0
            || (self.expected_buy_order_mask & self.expected_sell_order_mask) != 0
            || (self.expected_buy_order_mask == 0 && self.expected_sell_order_mask == 0)
            || (self.expected_buy_order_mask != 0 && self.expected_buy_price_units == 0)
            || (self.expected_sell_order_mask != 0 && self.expected_sell_price_units == 0)
        {
            return Err(Error::InvalidExpectation);
        }
        let required = owner_debit_atoms(
            self.expected_buy_price_units,
            self.price_scale,
            self.selected_fee_atoms,
        )?;
        if self.reserved_cash_atoms < required {
            return Err(Error::InsufficientCash);
        }
        Ok(())
    }
}

/// One exact receipt fragment after adapter membership authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedOwnerFragmentV1 {
    /// Canonical order-set index `0..64`.
    pub order_index: u8,
    /// Whether the order is a payer or payee.
    pub side: SettlementSideV1,
    /// Exact consideration represented by this receipt in price units.
    pub consideration_price_units: u128,
    /// True only on the unique receipt that exhausts this order's entitlement.
    pub completes_order: bool,
}

/// Mutable fixed-layout owner accumulator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementAccumulatorV1 {
    /// Immutable verifier-owned expectation.
    pub expectation: OwnerSettlementExpectationV1,
    /// Exact buy price units consumed so far.
    pub consumed_buy_price_units: u128,
    /// Exact sell price units consumed so far.
    pub consumed_sell_price_units: u128,
    /// Completed buy-order bitmap.
    pub completed_buy_order_mask: u64,
    /// Completed sell-order bitmap.
    pub completed_sell_order_mask: u64,
    /// Consumed receipt count.
    pub consumed_slice_count: u16,
    /// Zero while accumulating, one after final cash realization, two after
    /// the persistent owner row is retired.
    pub state: u8,
}

impl OwnerSettlementAccumulatorV1 {
    /// Create an empty accumulator from a complete verifier-owned expectation.
    pub fn new(expectation: OwnerSettlementExpectationV1) -> Result<Self> {
        expectation.validate()?;
        Ok(Self {
            expectation,
            consumed_buy_price_units: 0,
            consumed_sell_price_units: 0,
            completed_buy_order_mask: 0,
            completed_sell_order_mask: 0,
            consumed_slice_count: 0,
            state: 0,
        })
    }

    /// Apply one exact fragment without performing collateral rounding.
    pub fn consume(&mut self, fragment: AuthenticatedOwnerFragmentV1) -> Result<()> {
        self.validate()?;
        if self.state != 0 {
            return Err(Error::Terminal);
        }
        if fragment.consideration_price_units == 0 {
            return Err(Error::ZeroFragment);
        }
        let bit = order_bit(fragment.order_index)?;
        let mut next = *self;
        match fragment.side {
            SettlementSideV1::Buy => {
                if self.expectation.expected_buy_order_mask & bit == 0 {
                    return Err(Error::InvalidOrder);
                }
                next.consumed_buy_price_units = next
                    .consumed_buy_price_units
                    .checked_add(fragment.consideration_price_units)
                    .ok_or(Error::ArithmeticOverflow)?;
                if next.consumed_buy_price_units > self.expectation.expected_buy_price_units {
                    return Err(Error::InvariantViolation);
                }
                if fragment.completes_order {
                    if next.completed_buy_order_mask & bit != 0 {
                        return Err(Error::DuplicateCompletion);
                    }
                    next.completed_buy_order_mask |= bit;
                }
            }
            SettlementSideV1::Sell => {
                if self.expectation.expected_sell_order_mask & bit == 0 {
                    return Err(Error::InvalidOrder);
                }
                next.consumed_sell_price_units = next
                    .consumed_sell_price_units
                    .checked_add(fragment.consideration_price_units)
                    .ok_or(Error::ArithmeticOverflow)?;
                if next.consumed_sell_price_units > self.expectation.expected_sell_price_units {
                    return Err(Error::InvariantViolation);
                }
                if fragment.completes_order {
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
        if next.consumed_slice_count > self.expectation.expected_slice_count {
            return Err(Error::TooManyFragments);
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Convert exact owner totals once and stage the Position cash post-state.
    ///
    /// `position_cash_atoms` includes reserved cash, matching the authoritative
    /// Position representation. `position_reserved_cash_atoms` must cover the
    /// exact sum of this owner's reservation envelopes frozen in the row.
    pub fn finalize(
        &mut self,
        position_cash_atoms: Amount,
        position_reserved_cash_atoms: Amount,
    ) -> Result<OwnerSettlementDispositionV1> {
        self.validate()?;
        if self.state != 0 {
            return Err(Error::Terminal);
        }
        if self.consumed_slice_count != self.expectation.expected_slice_count
            || self.consumed_buy_price_units != self.expectation.expected_buy_price_units
            || self.consumed_sell_price_units != self.expectation.expected_sell_price_units
            || self.completed_buy_order_mask != self.expectation.expected_buy_order_mask
            || self.completed_sell_order_mask != self.expectation.expected_sell_order_mask
        {
            return Err(Error::Incomplete);
        }
        if position_reserved_cash_atoms < self.expectation.reserved_cash_atoms
            || position_cash_atoms < self.expectation.reserved_cash_atoms
        {
            return Err(Error::InsufficientCash);
        }
        let debit_atoms = owner_debit_atoms(
            self.expectation.expected_buy_price_units,
            self.expectation.price_scale,
            self.expectation.selected_fee_atoms,
        )?;
        let credit_atoms = owner_credit_atoms(
            self.expectation.expected_sell_price_units,
            self.expectation.price_scale,
        )?;
        let residue_price_units = owner_rounding_residue_price_units(
            self.expectation.expected_buy_price_units,
            self.expectation.expected_sell_price_units,
            self.expectation.price_scale,
        )?;
        let next_cash = position_cash_atoms
            .checked_sub(debit_atoms)
            .and_then(|value| value.checked_add(credit_atoms))
            .ok_or(Error::ArithmeticUnderflow)?;
        let next_reserved = position_reserved_cash_atoms
            .checked_sub(self.expectation.reserved_cash_atoms)
            .ok_or(Error::ArithmeticUnderflow)?;
        if next_reserved > next_cash {
            return Err(Error::InvariantViolation);
        }
        let released_cash_atoms = self
            .expectation
            .reserved_cash_atoms
            .checked_sub(debit_atoms)
            .ok_or(Error::InsufficientCash)?;
        let mut next = *self;
        next.state = 1;
        next.validate()?;
        *self = next;
        Ok(OwnerSettlementDispositionV1 {
            debit_atoms,
            credit_atoms,
            selected_fee_atoms: self.expectation.selected_fee_atoms,
            released_cash_atoms,
            residue_price_units,
            position_cash_atoms: next_cash,
            position_reserved_cash_atoms: next_reserved,
        })
    }

    /// Mark a finalized, fully consumed owner row as a permanent tombstone.
    pub fn retire(&mut self) -> Result<()> {
        self.validate()?;
        if self.state != 1 {
            return Err(Error::Incomplete);
        }
        self.state = 2;
        Ok(())
    }

    /// Validate canonical state and monotone bounds.
    pub fn validate(&self) -> Result<()> {
        self.expectation.validate()?;
        if self.state > 2
            || self.completed_buy_order_mask & !self.expectation.expected_buy_order_mask != 0
            || self.completed_sell_order_mask & !self.expectation.expected_sell_order_mask != 0
            || self.consumed_slice_count > self.expectation.expected_slice_count
            || self.consumed_buy_price_units > self.expectation.expected_buy_price_units
            || self.consumed_sell_price_units > self.expectation.expected_sell_price_units
        {
            return Err(Error::InvariantViolation);
        }
        if self.state != 0
            && (self.consumed_slice_count != self.expectation.expected_slice_count
                || self.consumed_buy_price_units != self.expectation.expected_buy_price_units
                || self.consumed_sell_price_units != self.expectation.expected_sell_price_units
                || self.completed_buy_order_mask != self.expectation.expected_buy_order_mask
                || self.completed_sell_order_mask != self.expectation.expected_sell_order_mask)
        {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }

    /// Encode the exact canonical persisted semantic body.
    pub fn encode_body(&self) -> Result<[u8; OWNER_SETTLEMENT_BODY_V1_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; OWNER_SETTLEMENT_BODY_V1_BYTES];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, &self.expectation.market)?;
        put(&mut output, &mut cursor, &self.expectation.epoch)?;
        put(&mut output, &mut cursor, &self.expectation.candidate)?;
        put(&mut output, &mut cursor, &self.expectation.owner)?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.owner_order_set_digest,
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.price_scale.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_buy_order_mask.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_sell_order_mask.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_slice_count.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_buy_price_units.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_sell_price_units.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.selected_fee_atoms.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.reserved_cash_atoms.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.consumed_buy_price_units.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.consumed_sell_price_units.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.completed_buy_order_mask.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.completed_sell_order_mask.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.consumed_slice_count.to_le_bytes(),
        )?;
        put(&mut output, &mut cursor, &[self.state])?;
        put(&mut output, &mut cursor, &[0; 3])?;
        if cursor != OWNER_SETTLEMENT_BODY_V1_BYTES {
            return Err(Error::InvariantViolation);
        }
        Ok(output)
    }

    /// Decode an exact hostile-byte-facing semantic body.
    pub fn decode_body(input: &[u8]) -> Result<Self> {
        if input.len() != OWNER_SETTLEMENT_BODY_V1_BYTES {
            return Err(Error::InvalidExpectation);
        }
        let mut cursor = 0_usize;
        let expectation = OwnerSettlementExpectationV1 {
            market: read_key(input, &mut cursor)?,
            epoch: read_key(input, &mut cursor)?,
            candidate: read_key(input, &mut cursor)?,
            owner: read_key(input, &mut cursor)?,
            owner_order_set_digest: read_key(input, &mut cursor)?,
            price_scale: read_u64(input, &mut cursor)?,
            expected_buy_order_mask: read_u64(input, &mut cursor)?,
            expected_sell_order_mask: read_u64(input, &mut cursor)?,
            expected_slice_count: read_u16(input, &mut cursor)?,
            expected_buy_price_units: read_u128(input, &mut cursor)?,
            expected_sell_price_units: read_u128(input, &mut cursor)?,
            selected_fee_atoms: read_u64(input, &mut cursor)?,
            reserved_cash_atoms: read_u64(input, &mut cursor)?,
        };
        let value = Self {
            expectation,
            consumed_buy_price_units: read_u128(input, &mut cursor)?,
            consumed_sell_price_units: read_u128(input, &mut cursor)?,
            completed_buy_order_mask: read_u64(input, &mut cursor)?,
            completed_sell_order_mask: read_u64(input, &mut cursor)?,
            consumed_slice_count: read_u16(input, &mut cursor)?,
            state: read_u8(input, &mut cursor)?,
        };
        if take(input, &mut cursor, 3)? != &[0; 3] || cursor != input.len() {
            return Err(Error::InvalidExpectation);
        }
        value.validate()?;
        Ok(value)
    }
}

/// Exact terminal owner cash disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementDispositionV1 {
    /// Whole collateral atoms debited for aggregate buy consideration plus fee.
    pub debit_atoms: Amount,
    /// Whole collateral atoms credited for aggregate sell consideration.
    pub credit_atoms: Amount,
    /// Selected fee atoms included in the debit.
    pub selected_fee_atoms: Amount,
    /// Reserved cash unlocked back to the owner's free balance.
    pub released_cash_atoms: Amount,
    /// Non-fee conversion slack contributed to the candidate rounding pot.
    pub residue_price_units: u128,
    /// Prospective total Position cash.
    pub position_cash_atoms: Amount,
    /// Prospective remaining Position reserved cash.
    pub position_reserved_cash_atoms: Amount,
}

/// Payer-side aggregate conversion, including a selected whole-atom fee.
pub fn owner_debit_atoms(
    buy_price_units: u128,
    price_scale: Amount,
    selected_fee_atoms: Amount,
) -> Result<Amount> {
    if price_scale == 0 {
        return Err(Error::InvalidExpectation);
    }
    let consideration = div_ceil(buy_price_units, u128::from(price_scale))?;
    let total = consideration
        .checked_add(u128::from(selected_fee_atoms))
        .ok_or(Error::ArithmeticOverflow)?;
    Amount::try_from(total).map_err(|_| Error::ArithmeticOverflow)
}

/// Payee-side aggregate conversion.
pub fn owner_credit_atoms(sell_price_units: u128, price_scale: Amount) -> Result<Amount> {
    if price_scale == 0 {
        return Err(Error::InvalidExpectation);
    }
    Amount::try_from(sell_price_units / u128::from(price_scale))
        .map_err(|_| Error::ArithmeticOverflow)
}

/// Exact non-fee slack from the one terminal owner rounding boundary.
pub fn owner_rounding_residue_price_units(
    buy_price_units: u128,
    sell_price_units: u128,
    price_scale: Amount,
) -> Result<u128> {
    if price_scale == 0 {
        return Err(Error::InvalidExpectation);
    }
    let scale = u128::from(price_scale);
    let debit = div_ceil(buy_price_units, scale)?;
    let credit = sell_price_units / scale;
    debit
        .checked_mul(scale)
        .and_then(|value| value.checked_sub(buy_price_units))
        .and_then(|payer| {
            sell_price_units
                .checked_sub(credit * scale)
                .and_then(|payee| payer.checked_add(payee))
        })
        .ok_or(Error::ArithmeticOverflow)
}

fn order_bit(index: u8) -> Result<u64> {
    if usize::from(index) >= MAX_ORDERS {
        return Err(Error::InvalidOrder);
    }
    1_u64
        .checked_shl(u32::from(index))
        .ok_or(Error::InvalidOrder)
}

fn div_ceil(numerator: u128, denominator: u128) -> Result<u128> {
    if denominator == 0 {
        return Err(Error::InvalidExpectation);
    }
    let quotient = numerator / denominator;
    if numerator % denominator == 0 {
        Ok(quotient)
    } else {
        quotient.checked_add(1).ok_or(Error::ArithmeticOverflow)
    }
}

fn put<const N: usize>(output: &mut [u8; N], cursor: &mut usize, bytes: &[u8]) -> Result<()> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(Error::ArithmeticOverflow)?;
    let target = output
        .get_mut(*cursor..end)
        .ok_or(Error::InvariantViolation)?;
    target.copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

fn take<'a>(input: &'a [u8], cursor: &mut usize, width: usize) -> Result<&'a [u8]> {
    let end = cursor
        .checked_add(width)
        .ok_or(Error::ArithmeticOverflow)?;
    let value = input.get(*cursor..end).ok_or(Error::InvalidExpectation)?;
    *cursor = end;
    Ok(value)
}

fn read_key(input: &[u8], cursor: &mut usize) -> Result<[u8; 32]> {
    let mut value = [0_u8; 32];
    value.copy_from_slice(take(input, cursor, 32)?);
    Ok(value)
}

fn read_u8(input: &[u8], cursor: &mut usize) -> Result<u8> {
    Ok(take(input, cursor, 1)?[0])
}

fn read_u16(input: &[u8], cursor: &mut usize) -> Result<u16> {
    let mut value = [0_u8; 2];
    value.copy_from_slice(take(input, cursor, 2)?);
    Ok(u16::from_le_bytes(value))
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut value = [0_u8; 8];
    value.copy_from_slice(take(input, cursor, 8)?);
    Ok(u64::from_le_bytes(value))
}

fn read_u128(input: &[u8], cursor: &mut usize) -> Result<u128> {
    let mut value = [0_u8; 16];
    value.copy_from_slice(take(input, cursor, 16)?);
    Ok(u128::from_le_bytes(value))
}
