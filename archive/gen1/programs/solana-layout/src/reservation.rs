//! Exact per-order reservation bytes and admission arithmetic.
//!
//! An order page commits public intent.  It does not own funds.  This module's
//! account is the per-order asset owner and binds one reservation to one
//! `(market, epoch, owner, position generation, order id)` identity.  The
//! Solana adapter must additionally authenticate its PDA and apply the
//! Position deltas atomically with the page write.
//!
//! ## Schema v2: the partial-fill ledger
//!
//! Since the PartialFillLedger wave the account also *is* the per-order
//! cumulative consumption ledger: `entitled_units` (the whole order's entitled
//! Egg atoms, stamped once at its first entitled slice), `consumed_units`
//! (monotone, bounded by it), and the reserved-zero fee landing zone
//! `fee_debited_atoms` / `fee_carry_numerator`.  The pinned invariant, held at
//! every transaction boundary and per cash and per outcome, is
//!
//! ```text
//! initial = consumed-so-far + remaining + released
//! ```
//!
//! with `released` zero until completion.  This is not a new account family
//! and not a policy fork: the frozen `GENERAL_CLEARING_POLICY_V1` digest is
//! unchanged and already admits partial fills.
//!
//! ## Schema v3: the payment ledger
//!
//! The VirtualMergeCredit wave split the ledger in two, because one seam
//! separates an end's Egg movement from its cash movement.  A virtual **merge**
//! consumes `mu` complete sets out of real sell ends and pays them from the
//! collateral that burning those sets releases — so the sets must be assembled
//! *before* the cash exists, and the closing order is deliver-every-leg, burn
//! `mu`, then pay.  A sell end therefore spends a window with its Eggs gone and
//! its credit not yet received, and one counter cannot describe both halves.
//!
//! [`ReservationAccount::consumed_units`] is that end's **quantity** ledger:
//! the units whose slice has moved.  [`ReservationAccount::paid_units`] is its
//! **cash** ledger: the units whose collateral leg has settled — the debit for
//! a payer, the credit for a payee.  Every seam but the merge settles both in
//! one transition, so the two counters are equal there by construction; the
//! merge is the one place `paid_units` lags, and it catches up exactly at the
//! payment that closes the order.  The invariants are
//!
//! ```text
//! paid_units <= consumed_units <= entitled_units
//! ```
//!
//! with both counters zero in `ACTIVE` and `RELEASED`, and — the one that
//! carries the wave — `paid_units == consumed_units` in `CONSUMED`: **no end
//! closes with a delivery it was never paid for.**  `CONSUMED` is now
//! completion of *both* ledgers, which is still exactly an empty remaining
//! envelope, because the completing delivery releases the remainder.

use super::{
    check_hash, check_padded_amounts, digest, order_id_rank, put_header, CodecError, Hash32,
    OrderSlot, Reader, Result, Writer, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES, ORDER_KIND_PORTFOLIO,
    ORDER_KIND_SINGLE,
};

/// The general clearing plane's reservation schema carrying the per-order
/// partial-fill and payment ledgers.
///
/// This is schema **v3** of the general reservation: v2's 610 bytes plus
/// `paid_units`, spliced into the ledger family after `consumed_units` rather
/// than appended past the reserved fee zone — the two counters are read
/// together on every seam, and the fee zone stays the tail it was declared as.
///
/// The version *byte* is 4: 1 was v1, 3 was v2, and 2 under
/// [`RESERVATION_ACCOUNT_TAG`] is claimed by
/// [`crate::direct_selection_v3::DIRECT_RESERVATION_V2_VERSION`] on the direct
/// plane.  Two schemas must not share one `(tag, version)` pair, and none do.
///
/// **The length no longer separates the two planes.** Until v3 the general
/// body was 610 bytes and the direct plane's was 618, and the pair was
/// separated by version *and* length.  One `u64` of payment ledger takes the
/// general body to 618 as well, so the `(tag, version)` pair is now the whole
/// uniqueness key — which is exactly the key the private `Reader` enforces,
/// before a single field is read.  Padding the body to keep the lengths apart would buy
/// a second discriminator with dead bytes; instead
/// `direct_selection_v3::reservation_v3_and_direct_v2_refuse_each_other`
/// proves both decoders refuse the other plane's exact bytes.
///
/// The one place a length was doing work beyond a decoder is an off-chain
/// `getProgramAccounts` scan — the keeper enumerates an epoch's reservation
/// archives by `(dataSize, memcmp(epoch at offset 66))` — and that stays exact
/// for a reason that does not depend on the length at all: **both planes put
/// their epoch account at the same PDA.** `seeds::epoch_pda(program_id,
/// market, epoch_index)` addresses a general `EpochAccount` and a
/// `DIRECT_EPOCH_V4_BYTES` body alike, so one `(market, epoch_index)` hosts one
/// plane and one only, and every reservation carrying a given epoch id belongs
/// to that plane.  A scan filtered on the epoch therefore cannot see the other
/// schema's bytes whatever their length.
///
/// V1 and v2 bytes refuse at the decoder — `Truncated` on the old lengths,
/// `WrongVersion` on the old version bytes — with no migration owed, the
/// `KernelAccount` v2 precedent.
pub const RESERVATION_ACCOUNT_VERSION: u8 = 4;
/// Reservation-account discriminator.
pub const RESERVATION_ACCOUNT_TAG: u8 = 19;
/// Exact fixed length of one reservation account.
///
/// V1's 570 bytes plus `entitled_units` (8), `consumed_units` (8),
/// `paid_units` (8), `fee_debited_atoms` (8), and `fee_carry_numerator` (16):
/// 618.
pub const RESERVATION_ACCOUNT_BYTES: usize =
    2 + (8 * 32) + (6 * 8) + 2 + 6 + (2 * MAX_OUTCOMES * 8) + 8 + 8 + 8 + 8 + 16;

/// Reservation owns an open order's entire admitted asset envelope.
pub const RESERVATION_STATE_ACTIVE: u8 = 0;
/// Reservation was returned before epoch freeze.
pub const RESERVATION_STATE_RELEASED: u8 = 1;
/// Reservation moved into a complete immutable settlement entitlement set.
///
/// Entered at the order's *first* entitled slice, which stamps
/// [`ReservationAccount::entitled_units`]; every later slice of the same order
/// re-derives that total and requires it equal.
pub const RESERVATION_STATE_ENTITLED: u8 = 2;
/// The entitlement consumed or refunded every remaining asset, and was paid
/// for every unit it consumed.
///
/// Entered exactly when `paid_units == consumed_units == entitled_units`.  The
/// quantity half is also exactly when the remaining envelope is empty — the
/// completing delivery releases the remainder in the same transition — and the
/// cash half is what a merge sell end waits for: its Eggs leave at its
/// delivery, its credit arrives after the burn, and only then does it close.
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
    /// Relation units this order is entitled to consume across *every* slice
    /// of the selected candidate's witness.
    ///
    /// Egg atoms for a single-Egg order, and — for a portfolio order — the
    /// filled lots times the sum of its coefficients, which is the same Egg
    /// atoms.  Zero until the order's first entitled slice stamps it from the
    /// digest-verified feed; every later slice of the same order re-derives
    /// the total and requires it equal, so a forged stamp cannot survive
    /// recomputation.
    pub entitled_units: u64,
    /// Relation units whose *quantity* leg has moved, in the same Egg-atom
    /// unit.
    ///
    /// Monotone and bounded by `entitled_units`; reaching it releases the
    /// remainder.  On every seam but the virtual merge it also settles the
    /// cash leg in the same transition, so `paid_units` moves with it there.
    pub consumed_units: u64,
    /// Relation units whose *cash* leg has settled — the debit for a payer
    /// end, the credit for a payee end.
    ///
    /// Monotone and bounded by `consumed_units`: an end can never be paid for
    /// more than it has moved.  Equal to it everywhere except inside one
    /// virtual-merge window, where a sell end's Eggs land in the epoch pot at
    /// its delivery and its credit is only funded once the pot has burned the
    /// `mu` complete sets they complete.  `CONSUMED` requires the two equal,
    /// which is what makes "closed" mean "paid".
    pub paid_units: u64,
    /// Fee atoms already debited from this envelope.
    ///
    /// RESERVED for the adopted composite fee base.  V3 semantics validate it
    /// zero: the five-plus-one zero gates stand and no seam writes a fee.
    pub fee_debited_atoms: u64,
    /// Sub-atom fee carry numerator held for this owner identity.
    ///
    /// RESERVED for the adopted composite fee base, alongside
    /// [`Self::fee_debited_atoms`], and validated zero in v3 semantics.
    pub fee_carry_numerator: u128,
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
            entitled_units: 0,
            consumed_units: 0,
            paid_units: 0,
            fee_debited_atoms: 0,
            fee_carry_numerator: 0,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate identity, phase, side, padding, and remaining-asset bounds.
    pub fn validate(&self) -> Result<()> {
        self.validate_with_identity(canonical_reservation_id(
            self.market,
            self.epoch,
            self.owner,
            self.position_generation,
            self.order_id,
        ))
    }

    /// Validate every body invariant against one schema-owned identity.
    ///
    /// Kept crate-visible so a fresh account version can reuse the economic
    /// state machine while refusing this historical schema's identity domain.
    pub(crate) fn validate_with_identity(&self, expected_reservation: Hash32) -> Result<()> {
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
        if self.reservation != expected_reservation {
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
        /* The fee landing zone is reserved, not live: v3 semantics validate
         * both fields zero, so no byte of a fee can be persisted here until a
         * frozen fee base and a named recipient exist. */
        if self.fee_debited_atoms != 0 || self.fee_carry_numerator != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        /* The two ledgers, ordered.  Quantity is bounded by the stamp, and
         * cash is bounded by quantity: an end may lag its payment behind its
         * delivery — the virtual merge does, by exactly one burn — but it can
         * never be paid for a unit it has not moved. */
        if self.consumed_units > self.entitled_units || self.paid_units > self.consumed_units {
            return Err(CodecError::AggregateClosureMismatch);
        }
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
        /* The ledgers' phase coherence.  An *unstamped* ledger — every counter
         * zero — is the pre-ledger body: it is what an untouched reservation
         * carries, what a release returns, and what the atomic portfolio
         * full-pair route consumes, which never partially fills and so never
         * stamps.  A *stamped* ledger (`entitled_units != 0`) is the general
         * per-slice plane's, where CONSUMED is completion of *both* counters. */
        match self.state {
            RESERVATION_STATE_ACTIVE => {
                if self.release_generation != 0
                    || self.remaining_cash_atoms != self.initial_cash_atoms
                    || self.remaining_internal != self.initial_internal
                    || self.entitled_units != 0
                    || self.consumed_units != 0
                    || self.paid_units != 0
                {
                    return Err(CodecError::AggregateClosureMismatch);
                }
            }
            RESERVATION_STATE_RELEASED => {
                if self.release_generation <= self.order_generation
                    || !self.remaining_is_zero()
                    || self.entitled_units != 0
                    || self.consumed_units != 0
                    || self.paid_units != 0
                {
                    return Err(CodecError::AggregateClosureMismatch);
                }
            }
            RESERVATION_STATE_ENTITLED => {
                if self.release_generation != 0 {
                    return Err(CodecError::NonCanonicalPadding);
                }
                /* Completing *both* ledgers is completion, which is CONSUMED;
                 * an ENTITLED account that claims it is an unclosed remainder.
                 * A merge sell end that has delivered but not been paid has
                 * only completed the quantity half, and stays ENTITLED — which
                 * is the whole reason the two counters are separate. */
                if self.entitled_units != 0
                    && self.consumed_units == self.entitled_units
                    && self.paid_units == self.entitled_units
                {
                    return Err(CodecError::AggregateClosureMismatch);
                }
            }
            RESERVATION_STATE_CONSUMED => {
                if self.release_generation != 0
                    || !self.remaining_is_zero()
                    || self.consumed_units != self.entitled_units
                    || self.paid_units != self.consumed_units
                {
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

    /// Return the first-touch entitled post-state without mutating the
    /// original: `ACTIVE → ENTITLED` with the order's whole entitled total
    /// stamped once.
    ///
    /// The total is the *order's*, not the slice's: every later slice of the
    /// same order re-derives it from the digest-verified feed and requires
    /// [`Self::requires_stamp`], so no caller can widen an order's admission
    /// by entering through a different slice.
    pub fn entitled(&self, entitled_units: u64) -> Result<Self> {
        self.validate()?;
        if self.state != RESERVATION_STATE_ACTIVE || entitled_units == 0 {
            return Err(CodecError::MismatchedBinding);
        }
        let mut next = *self;
        next.entitled_units = entitled_units;
        next.state = RESERVATION_STATE_ENTITLED;
        next.validate()?;
        Ok(next)
    }

    /// Require an already-entitled account to carry exactly this stamp.
    pub fn requires_stamp(&self, entitled_units: u64) -> Result<()> {
        if self.state != RESERVATION_STATE_ENTITLED || self.entitled_units != entitled_units {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Relation units this reservation may still consume.
    pub fn unconsumed_units(&self) -> u64 {
        self.entitled_units - self.consumed_units
    }

    /// Relation units this reservation has moved but not yet been paid for.
    ///
    /// Zero on every end but a merge sell end between its delivery and its
    /// credit; the subtraction is unchecked for the same reason
    /// [`Self::unconsumed_units`]'s is — `validate` pins `paid_units <=
    /// consumed_units` on every value that can be decoded or encoded.
    pub fn unpaid_units(&self) -> u64 {
        self.consumed_units - self.paid_units
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
        w.u64(self.entitled_units)?;
        w.u64(self.consumed_units)?;
        w.u64(self.paid_units)?;
        w.u64(self.fee_debited_atoms)?;
        w.u128(self.fee_carry_numerator)?;
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
            entitled_units: r.u64()?,
            consumed_units: r.u64()?,
            paid_units: r.u64()?,
            fee_debited_atoms: r.u64()?,
            fee_carry_numerator: r.u128()?,
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
    fn the_v3_body_is_exactly_six_hundred_eighteen_bytes_and_v1_v2_bytes_refuse() {
        assert_eq!(RESERVATION_ACCOUNT_BYTES, 618);
        assert_eq!(RESERVATION_ACCOUNT_VERSION, 4);
        let value =
            account(ReservationPlan::for_order(&single(1, 7, 2_500), 3, 10_000, 9).unwrap());
        let mut bytes = [0u8; RESERVATION_ACCOUNT_BYTES];
        assert_eq!(value.encode(&mut bytes), Ok(RESERVATION_ACCOUNT_BYTES));
        // The five ledger words are the tail past v1's 570-byte boundary, all
        // zero on an ACTIVE reservation, and nothing before them moved.
        assert_eq!(&bytes[570..618], &[0u8; 48]);
        // `paid_units` sits inside the ledger family, not past the reserved
        // fee zone: the two counters that are read together are adjacent.
        assert_eq!(&bytes[586..594], &0u64.to_le_bytes());

        /* V1 and v2 bytes both refuse, on length where the length moved and on
         * the version byte where it did not.  No migration is owed and none is
         * offered. */
        assert_eq!(
            ReservationAccount::decode(&bytes[..570]),
            Err(CodecError::Truncated)
        );
        assert_eq!(
            ReservationAccount::decode(&bytes[..610]),
            Err(CodecError::Truncated)
        );
        for stale in [1u8, 3] {
            let mut old_version = bytes;
            old_version[1] = stale;
            assert_eq!(
                ReservationAccount::decode(&old_version),
                Err(CodecError::WrongVersion)
            );
        }
    }

    #[test]
    fn the_ledger_counters_bind_every_phase_and_the_fee_zone_stays_zero() {
        let value =
            account(ReservationPlan::for_order(&single(1, 7, 2_500), 3, 10_000, 9).unwrap());

        // The reserved fee landing zone is validated zero in v3 semantics.
        for mut hostile in [value, value] {
            hostile.fee_debited_atoms = 1;
            assert_eq!(hostile.validate(), Err(CodecError::NonCanonicalPadding));
        }
        let mut carry = value;
        carry.fee_carry_numerator = 1;
        assert_eq!(carry.validate(), Err(CodecError::NonCanonicalPadding));

        // An ACTIVE reservation carries no stamp at all.
        let mut stamped_active = value;
        stamped_active.entitled_units = 7;
        assert_eq!(
            stamped_active.validate(),
            Err(CodecError::AggregateClosureMismatch)
        );

        // The first touch stamps once; a second stamp, a zero stamp, and a
        // stamp on a non-ACTIVE account all refuse.
        let entitled = value.entitled(7).unwrap();
        assert_eq!(entitled.state, RESERVATION_STATE_ENTITLED);
        assert_eq!(entitled.entitled_units, 7);
        assert_eq!(entitled.consumed_units, 0);
        assert_eq!(entitled.unconsumed_units(), 7);
        assert_eq!(entitled.requires_stamp(7), Ok(()));
        assert_eq!(
            entitled.requires_stamp(6),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(entitled.entitled(7), Err(CodecError::MismatchedBinding));
        assert_eq!(value.entitled(0), Err(CodecError::MismatchedBinding));
        assert_eq!(value.requires_stamp(7), Err(CodecError::MismatchedBinding));

        // Consumption beyond the stamp refuses, and completing *both* ledgers
        // while still ENTITLED refuses too: that is what completion means.
        let mut over = entitled;
        over.consumed_units = 8;
        assert_eq!(over.validate(), Err(CodecError::AggregateClosureMismatch));
        let mut complete_but_entitled = entitled;
        complete_but_entitled.consumed_units = 7;
        complete_but_entitled.paid_units = 7;
        assert_eq!(
            complete_but_entitled.validate(),
            Err(CodecError::AggregateClosureMismatch)
        );
        let mut partway = entitled;
        partway.consumed_units = 3;
        partway.paid_units = 3;
        partway.remaining_internal[1] = 4;
        assert_eq!(partway.validate(), Ok(()));

        /* The merge window, and the only place it is legal: an end whose Eggs
         * have all been delivered into the pot and whose credit the burn has
         * not funded yet stays ENTITLED with the quantity ledger full and the
         * payment ledger empty. */
        let mut delivered_unpaid = entitled;
        delivered_unpaid.consumed_units = 7;
        delivered_unpaid.remaining_internal = [0; MAX_OUTCOMES];
        assert_eq!(delivered_unpaid.validate(), Ok(()));
        assert_eq!(delivered_unpaid.unpaid_units(), 7);
        // Paid past delivered is the unbacked direction and never validates.
        let mut overpaid = entitled;
        overpaid.consumed_units = 3;
        overpaid.paid_units = 4;
        assert_eq!(
            overpaid.validate(),
            Err(CodecError::AggregateClosureMismatch)
        );

        // CONSUMED is `paid == consumed == entitled` and an empty envelope: a
        // closed end was paid for every unit it moved.
        let mut consumed = delivered_unpaid;
        consumed.paid_units = 7;
        consumed.state = RESERVATION_STATE_CONSUMED;
        assert_eq!(consumed.validate(), Ok(()));
        let mut short = consumed;
        short.consumed_units = 6;
        short.paid_units = 6;
        assert_eq!(short.validate(), Err(CodecError::AggregateClosureMismatch));
        let mut unpaid_close = consumed;
        unpaid_close.paid_units = 6;
        assert_eq!(
            unpaid_close.validate(),
            Err(CodecError::AggregateClosureMismatch)
        );

        // A release never carries a ledger: it is the never-entitled exit.
        let mut released = value.released(5).unwrap();
        assert_eq!(released.entitled_units, 0);
        released.entitled_units = 7;
        assert_eq!(
            released.validate(),
            Err(CodecError::AggregateClosureMismatch)
        );
        let mut paid_release = value.released(5).unwrap();
        paid_release.paid_units = 1;
        assert_eq!(
            paid_release.validate(),
            Err(CodecError::AggregateClosureMismatch)
        );

        // The counters survive the codec byte for byte.
        let mut bytes = [0u8; RESERVATION_ACCOUNT_BYTES];
        partway.encode(&mut bytes).unwrap();
        assert_eq!(ReservationAccount::decode(&bytes), Ok(partway));
        assert_eq!(&bytes[570..578], &7u64.to_le_bytes());
        assert_eq!(&bytes[578..586], &3u64.to_le_bytes());
        assert_eq!(&bytes[586..594], &3u64.to_le_bytes());
        // A hostile nonzero fee word in the reserved tail refuses at decode.
        bytes[594] = 1;
        assert_eq!(
            ReservationAccount::decode(&bytes),
            Err(CodecError::NonCanonicalPadding)
        );
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
