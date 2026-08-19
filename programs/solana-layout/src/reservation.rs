//! Exact per-order reservation bytes and admission arithmetic.
//!
//! An order page commits public intent.  It does not own funds.  This module's
//! account is the per-order asset owner and binds one reservation to one
//! `(market, epoch, owner, position generation, order id)` identity.  The
//! Solana adapter must additionally authenticate its PDA and apply the
//! Position deltas atomically with the page write.

use super::{
    check_hash, check_padded_amounts, digest, order_id_rank, put_header, CodecError, Hash32,
    OrderSlot, Reader, Result, Writer, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES, ORDER_KIND_PORTFOLIO,
    ORDER_KIND_SINGLE,
};

/// First reservation-account schema.
pub const RESERVATION_ACCOUNT_VERSION: u8 = 1;
/// Reservation-account discriminator.
pub const RESERVATION_ACCOUNT_TAG: u8 = 19;
/// Exact fixed length of one reservation account.
pub const RESERVATION_ACCOUNT_BYTES: usize =
    2 + (8 * 32) + (6 * 8) + 2 + 6 + (2 * MAX_OUTCOMES * 8);

/// Reservation owns an open order's entire admitted asset envelope.
pub const RESERVATION_STATE_ACTIVE: u8 = 0;
/// Reservation was returned before epoch freeze.
pub const RESERVATION_STATE_RELEASED: u8 = 1;
/// Reservation moved into a complete immutable settlement entitlement set.
///
/// No production transition writes this state yet.
pub const RESERVATION_STATE_ENTITLED: u8 = 2;
/// The entitlement consumed or refunded every remaining asset.
///
/// No production transition writes this state yet.
pub const RESERVATION_STATE_CONSUMED: u8 = 3;

/// Exact asset envelope an order must reserve at admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReservationPlan {
    /// Collateral atoms reserved by a buy, including fee headroom.
    pub cash_atoms: u64,
    /// Egg atoms reserved by a sell, canonically padded to the market width.
    pub internal: [u64; MAX_OUTCOMES],
    /// Signed maximum fee, persisted separately from the total cash envelope.
    pub max_fee_atoms: u64,
    /// Active market outcome width.
    pub outcome_count: u8,
    /// Frozen order family discriminator.
    pub order_kind: u8,
    /// Order side: zero buy, one sell.
    pub side: u8,
}

impl ReservationPlan {
    /// Compute the exact account-sized reservation for one admitted order.
    ///
    /// A single-Egg buy reserves `ceil(quantity * limit / price_scale)` plus
    /// its signed fee cap.  A portfolio buy's persisted bound is already in
    /// collateral atoms per lot, so it reserves `lots * bound` plus the cap.
    /// Sells reserve their exact Egg vector; their proposed fee direction is a
    /// withholding from proceeds, not a debit from free cash.
    pub fn for_order(
        slot: &OrderSlot,
        outcome_count: u8,
        price_scale: u64,
        max_fee_atoms: u64,
    ) -> Result<Self> {
        if outcome_count < 2 || usize::from(outcome_count) > MAX_OUTCOMES || price_scale == 0 {
            return Err(CodecError::InvalidCount);
        }
        let mut internal = [0u64; MAX_OUTCOMES];
        let (cash_atoms, order_kind, side) = match slot {
            OrderSlot::Single(order) => {
                order.validate()?;
                if order.outcome >= outcome_count {
                    return Err(CodecError::MismatchedBinding);
                }
                match order.side {
                    0 => {
                        let units = (order.quantity as u128)
                            .checked_mul(order.limit as u128)
                            .ok_or(CodecError::ArithmeticOverflow)?;
                        let consideration = ceil_price_units(units, price_scale)?;
                        (
                            consideration
                                .checked_add(max_fee_atoms)
                                .ok_or(CodecError::ArithmeticOverflow)?,
                            ORDER_KIND_SINGLE,
                            order.side,
                        )
                    }
                    1 => {
                        internal[usize::from(order.outcome)] = order.quantity;
                        (0, ORDER_KIND_SINGLE, order.side)
                    }
                    _ => return Err(CodecError::InvalidEnum),
                }
            }
            OrderSlot::Portfolio(order) => {
                order.validate_on_scale(price_scale)?;
                if order.active_len > outcome_count {
                    return Err(CodecError::MismatchedBinding);
                }
                match order.side {
                    0 => {
                        let consideration = order
                            .lots
                            .checked_mul(order.limit_collateral_per_lot)
                            .ok_or(CodecError::ArithmeticOverflow)?;
                        (
                            consideration
                                .checked_add(max_fee_atoms)
                                .ok_or(CodecError::ArithmeticOverflow)?,
                            ORDER_KIND_PORTFOLIO,
                            order.side,
                        )
                    }
                    1 => {
                        let mut i = 0usize;
                        while i < usize::from(order.active_len) {
                            internal[i] = order
                                .lots
                                .checked_mul(order.coefficients[i])
                                .ok_or(CodecError::ArithmeticOverflow)?;
                            i += 1;
                        }
                        (0, ORDER_KIND_PORTFOLIO, order.side)
                    }
                    _ => return Err(CodecError::InvalidEnum),
                }
            }
            OrderSlot::Empty | OrderSlot::Tombstone(_) => return Err(CodecError::InvalidEnum),
        };
        check_padded_amounts(&internal, usize::from(outcome_count))?;
        Ok(Self {
            cash_atoms,
            internal,
            max_fee_atoms,
            outcome_count,
            order_kind,
            side,
        })
    }
}

/// Persisted reservation and its remaining asset ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReservationAccount {
    /// Canonical digest of the reservation identity tuple.
    pub reservation: Hash32,
    /// Market identity.
    pub market: Hash32,
    /// Epoch identity.
    pub epoch: Hash32,
    /// Position owner identity.
    pub owner: Hash32,
    /// Canonical positional order identity.
    pub order_id: Hash32,
    /// Frozen price-grid identity used for admission.
    pub price_grid: Hash32,
    /// Immutable Terms identity owning the exact Egg basis.
    pub terms: Hash32,
    /// Frozen policy identity under which the order may settle.
    pub policy: Hash32,
    /// Position generation, preventing a close/reopen alias.
    pub position_generation: u64,
    /// Generation carried by the live order record.
    pub order_generation: u64,
    /// Initial collateral envelope.
    pub initial_cash_atoms: u64,
    /// Collateral still owned by this reservation.
    pub remaining_cash_atoms: u64,
    /// Signed fee ceiling included in the buy-side envelope.
    pub max_fee_atoms: u64,
    /// Release generation; zero until cancellation/lapse.
    pub release_generation: u64,
    /// Page index recomputed from `order_id`.
    pub page_index: u16,
    /// Active market outcome width.
    pub outcome_count: u8,
    /// Order family discriminator.
    pub order_kind: u8,
    /// Order side: zero buy, one sell.
    pub side: u8,
    /// Ownership phase; see `RESERVATION_STATE_*`.
    pub state: u8,
    /// Stored reservation PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; zero in V1.
    pub flags: u8,
    /// Initial Egg envelope.
    pub initial_internal: [u64; MAX_OUTCOMES],
    /// Eggs still owned by this reservation.
    pub remaining_internal: [u64; MAX_OUTCOMES],
}

impl ReservationAccount {
    /// Construct an active reservation from an already checked plan.
    #[allow(clippy::too_many_arguments)]
    pub fn active(
        market: Hash32,
        epoch: Hash32,
        owner: Hash32,
        order_id: Hash32,
        price_grid: Hash32,
        terms: Hash32,
        policy: Hash32,
        position_generation: u64,
        order_generation: u64,
        page_index: u16,
        stored_bump: u8,
        plan: ReservationPlan,
    ) -> Result<Self> {
        let value = Self {
            reservation: canonical_reservation_id(
                market,
                epoch,
                owner,
                position_generation,
                order_id,
            ),
            market,
            epoch,
            owner,
            order_id,
            price_grid,
            terms,
            policy,
            position_generation,
            order_generation,
            initial_cash_atoms: plan.cash_atoms,
            remaining_cash_atoms: plan.cash_atoms,
            max_fee_atoms: plan.max_fee_atoms,
            release_generation: 0,
            page_index,
            outcome_count: plan.outcome_count,
            order_kind: plan.order_kind,
            side: plan.side,
            state: RESERVATION_STATE_ACTIVE,
            stored_bump,
            flags: 0,
            initial_internal: plan.internal,
            remaining_internal: plan.internal,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate identity, phase, side, padding, and remaining-asset bounds.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.market,
            self.epoch,
            self.owner,
            self.order_id,
            self.price_grid,
            self.terms,
            self.policy,
        ] {
            check_hash(identity)?;
        }
        if self.reservation
            != canonical_reservation_id(
                self.market,
                self.epoch,
                self.owner,
                self.position_generation,
                self.order_id,
            )
        {
            return Err(CodecError::NonCanonicalIdentity);
        }
        let rank = order_id_rank(self.order_id)?;
        let page_index = (rank - 1) / MAX_ORDERS_PER_PAGE as u64;
        if page_index != self.page_index as u64 {
            return Err(CodecError::MismatchedBinding);
        }
        if self.outcome_count < 2 || usize::from(self.outcome_count) > MAX_OUTCOMES {
            return Err(CodecError::InvalidCount);
        }
        if !matches!(self.order_kind, ORDER_KIND_SINGLE | ORDER_KIND_PORTFOLIO)
            || self.side > 1
            || self.state > RESERVATION_STATE_CONSUMED
            || self.flags != 0
        {
            return Err(CodecError::InvalidEnum);
        }
        check_padded_amounts(&self.initial_internal, usize::from(self.outcome_count))?;
        check_padded_amounts(&self.remaining_internal, usize::from(self.outcome_count))?;
        if self.remaining_cash_atoms > self.initial_cash_atoms {
            return Err(CodecError::AggregateClosureMismatch);
        }
        let mut any_initial_internal = false;
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            if self.remaining_internal[i] > self.initial_internal[i] {
                return Err(CodecError::AggregateClosureMismatch);
            }
            any_initial_internal |= self.initial_internal[i] != 0;
            i += 1;
        }
        match self.side {
            0 => {
                if any_initial_internal || self.initial_cash_atoms < self.max_fee_atoms {
                    return Err(CodecError::AggregateClosureMismatch);
                }
            }
            1 => {
                if !any_initial_internal
                    || self.initial_cash_atoms != 0
                    || self.remaining_cash_atoms != 0
                {
                    return Err(CodecError::AggregateClosureMismatch);
                }
            }
            _ => return Err(CodecError::InvalidEnum),
        }
        match self.state {
            RESERVATION_STATE_ACTIVE => {
                if self.release_generation != 0
                    || self.remaining_cash_atoms != self.initial_cash_atoms
                    || self.remaining_internal != self.initial_internal
                {
                    return Err(CodecError::AggregateClosureMismatch);
                }
            }
            RESERVATION_STATE_RELEASED => {
                if self.release_generation <= self.order_generation || !self.remaining_is_zero() {
                    return Err(CodecError::AggregateClosureMismatch);
                }
            }
            RESERVATION_STATE_ENTITLED => {
                if self.release_generation != 0 {
                    return Err(CodecError::NonCanonicalPadding);
                }
            }
            RESERVATION_STATE_CONSUMED => {
                if self.release_generation != 0 || !self.remaining_is_zero() {
                    return Err(CodecError::AggregateClosureMismatch);
                }
            }
            _ => return Err(CodecError::InvalidEnum),
        }
        Ok(())
    }

    /// Whether no asset remains under this account's ownership.
    pub fn remaining_is_zero(&self) -> bool {
        if self.remaining_cash_atoms != 0 {
            return false;
        }
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            if self.remaining_internal[i] != 0 {
                return false;
            }
            i += 1;
        }
        true
    }

    /// Return a released post-state without mutating the original.
    pub fn released(&self, release_generation: u64) -> Result<Self> {
        self.validate()?;
        if self.state != RESERVATION_STATE_ACTIVE || release_generation <= self.order_generation {
            return Err(CodecError::MismatchedBinding);
        }
        let mut next = *self;
        next.remaining_cash_atoms = 0;
        next.remaining_internal = [0; MAX_OUTCOMES];
        next.release_generation = release_generation;
        next.state = RESERVATION_STATE_RELEASED;
        next.validate()?;
        Ok(next)
    }

    /// Encode exactly [`RESERVATION_ACCOUNT_BYTES`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < RESERVATION_ACCOUNT_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, RESERVATION_ACCOUNT_TAG, RESERVATION_ACCOUNT_VERSION)?;
        for identity in [
            self.reservation,
            self.market,
            self.epoch,
            self.owner,
            self.order_id,
            self.price_grid,
            self.terms,
            self.policy,
        ] {
            w.hash(identity)?;
        }
        w.u64(self.position_generation)?;
        w.u64(self.order_generation)?;
        w.u64(self.initial_cash_atoms)?;
        w.u64(self.remaining_cash_atoms)?;
        w.u64(self.max_fee_atoms)?;
        w.u64(self.release_generation)?;
        w.u16(self.page_index)?;
        w.u8(self.outcome_count)?;
        w.u8(self.order_kind)?;
        w.u8(self.side)?;
        w.u8(self.state)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        w.amounts(&self.initial_internal)?;
        w.amounts(&self.remaining_internal)?;
        Ok(w.at)
    }

    /// Parse exactly [`RESERVATION_ACCOUNT_BYTES`] hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            RESERVATION_ACCOUNT_TAG,
            RESERVATION_ACCOUNT_VERSION,
            RESERVATION_ACCOUNT_BYTES,
        )?;
        let value = Self {
            reservation: r.hash()?,
            market: r.hash()?,
            epoch: r.hash()?,
            owner: r.hash()?,
            order_id: r.hash()?,
            price_grid: r.hash()?,
            terms: r.hash()?,
            policy: r.hash()?,
            position_generation: r.u64()?,
            order_generation: r.u64()?,
            initial_cash_atoms: r.u64()?,
            remaining_cash_atoms: r.u64()?,
            max_fee_atoms: r.u64()?,
            release_generation: r.u64()?,
            page_index: r.u16()?,
            outcome_count: r.u8()?,
            order_kind: r.u8()?,
            side: r.u8()?,
            state: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
            initial_internal: r.amounts()?,
            remaining_internal: r.amounts()?,
        };
        r.done()?;
        value.validate()?;
        Ok(value)
    }
}

/// Canonical per-order reservation identity.
pub fn canonical_reservation_id(
    market: Hash32,
    epoch: Hash32,
    owner: Hash32,
    position_generation: u64,
    order_id: Hash32,
) -> Hash32 {
    digest(
        b"dragons-clutch/reservation/v1",
        &[
            &market.0,
            &epoch.0,
            &owner.0,
            &position_generation.to_le_bytes(),
            &order_id.0,
        ],
    )
}

fn ceil_price_units(price_units: u128, price_scale: u64) -> Result<u64> {
    let scale = price_scale as u128;
    let atoms = price_units
        .checked_add(scale - 1)
        .ok_or(CodecError::ArithmeticOverflow)?
        / scale;
    u64::try_from(atoms).map_err(|_| CodecError::ArithmeticOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{canonical_order_id, OrderRecord, PortfolioRecord};

    fn h(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn single(side: u8, quantity: u64, limit: u64) -> OrderSlot {
        OrderSlot::Single(OrderRecord {
            owner: h(3),
            order_id: canonical_order_id(17),
            outcome: 1,
            side,
            quantity,
            limit,
            minimum_fill: 0,
            flags: 0,
            generation: 4,
            expiry_epoch: 9,
        })
    }

    fn portfolio(side: u8, lots: u64, coefficients: [u64; MAX_OUTCOMES]) -> OrderSlot {
        OrderSlot::Portfolio(PortfolioRecord {
            owner: h(3),
            order_id: canonical_order_id(17),
            side,
            active_len: 3,
            flags: 0,
            coefficients,
            lots,
            limit_collateral_per_lot: 9,
            minimum_fill_lots: 0,
            generation: 4,
            expiry_epoch: 9,
        })
    }

    fn account(plan: ReservationPlan) -> ReservationAccount {
        ReservationAccount::active(
            h(1),
            h(2),
            h(3),
            canonical_order_id(17),
            h(4),
            h(5),
            h(6),
            7,
            4,
            1,
            6,
            plan,
        )
        .unwrap()
    }

    #[test]
    fn single_buy_uses_one_ceil_and_fee_headroom() {
        let plan = ReservationPlan::for_order(&single(0, 3, 2_501), 3, 10_000, 2).unwrap();
        assert_eq!(plan.cash_atoms, 3);
        assert_eq!(plan.internal, [0; MAX_OUTCOMES]);
        assert_eq!(plan.max_fee_atoms, 2);
    }

    #[test]
    fn sell_moves_exact_claims_and_portfolio_products_must_fit_u64() {
        let single = ReservationPlan::for_order(&single(1, 7, 2_500), 3, 10_000, 9).unwrap();
        assert_eq!(single.cash_atoms, 0);
        assert_eq!(single.internal[1], 7);

        let mut coefficients = [0u64; MAX_OUTCOMES];
        coefficients[0] = 3;
        coefficients[2] = 5;
        let basket =
            ReservationPlan::for_order(&portfolio(1, 4, coefficients), 3, 10_000, 9).unwrap();
        assert_eq!(&basket.internal[..3], &[12, 0, 20]);

        coefficients[0] = u64::MAX;
        assert_eq!(
            ReservationPlan::for_order(&portfolio(1, 2, coefficients), 3, 10_000, 0),
            Err(CodecError::ArithmeticOverflow)
        );
    }

    #[test]
    fn portfolio_buy_is_exact_atoms_plus_fee() {
        let mut coefficients = [0u64; MAX_OUTCOMES];
        coefficients[0] = 3;
        coefficients[2] = 5;
        let plan =
            ReservationPlan::for_order(&portfolio(0, 4, coefficients), 3, 10_000, 2).unwrap();
        assert_eq!(plan.cash_atoms, 38);
        assert_eq!(plan.internal, [0; MAX_OUTCOMES]);
    }

    #[test]
    fn codec_binds_identity_page_policy_grid_and_remaining_assets() {
        let value =
            account(ReservationPlan::for_order(&single(1, 7, 2_500), 3, 10_000, 9).unwrap());
        let mut bytes = [0u8; RESERVATION_ACCOUNT_BYTES];
        assert_eq!(value.encode(&mut bytes), Ok(RESERVATION_ACCOUNT_BYTES));
        assert_eq!(ReservationAccount::decode(&bytes), Ok(value));

        let mut wrong_page = value;
        wrong_page.page_index = 0;
        assert_eq!(wrong_page.validate(), Err(CodecError::MismatchedBinding));
        let mut wrong_policy = value;
        wrong_policy.policy = Hash32::ZERO;
        assert_eq!(wrong_policy.validate(), Err(CodecError::ZeroIdentity));
        let mut grown = value;
        grown.remaining_internal[1] = 8;
        assert_eq!(grown.validate(), Err(CodecError::AggregateClosureMismatch));
    }

    #[test]
    fn release_is_consumptive_and_replay_refuses() {
        let value =
            account(ReservationPlan::for_order(&single(1, 7, 2_500), 3, 10_000, 9).unwrap());
        let released = value.released(5).unwrap();
        assert!(released.remaining_is_zero());
        assert_eq!(released.state, RESERVATION_STATE_RELEASED);
        assert_eq!(released.released(6), Err(CodecError::MismatchedBinding));
        assert_eq!(value.released(4), Err(CodecError::MismatchedBinding));
    }

    #[test]
    fn hostile_state_and_padding_bytes_refuse() {
        let value =
            account(ReservationPlan::for_order(&single(0, 3, 2_501), 3, 10_000, 2).unwrap());
        let mut bytes = [0u8; RESERVATION_ACCOUNT_BYTES];
        value.encode(&mut bytes).unwrap();
        let state_offset = 2 + (8 * 32) + (6 * 8) + 2 + 3;
        bytes[state_offset] = 9;
        assert_eq!(
            ReservationAccount::decode(&bytes),
            Err(CodecError::InvalidEnum)
        );
        value.encode(&mut bytes).unwrap();
        let initial_internal = 2 + (8 * 32) + (6 * 8) + 2 + 6;
        bytes[initial_internal + (15 * 8)] = 1;
        assert_eq!(
            ReservationAccount::decode(&bytes),
            Err(CodecError::NonCanonicalPadding)
        );
    }
}
