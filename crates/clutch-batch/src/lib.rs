#![no_std]
#![forbid(unsafe_code)]

//! Deterministic frequent-batch relations.
//!
//! This crate is deliberately independent of Solana and of any account or
//! matching-service representation.  A book is a fixed array, its policy is
//! frozen at construction, and a candidate can be recomputed by any verifier.
//! Nothing here is an optimality claim: an accepted candidate is only ever the
//! best valid submitted candidate under the frozen, explicit tie rule.
//!
//! The crate holds three relations, and they are not the same object:
//!
//! * the **scalar** relation in this module ([`FixedBook`]) clears one grid tick
//!   over side totals with owner and outcome erased.  It is retained unchanged
//!   as a regression lab, and its known defect (§P1-B of the adversarial review:
//!   matched volume that no executable transfer can realize) is exactly what the
//!   coupled relation repairs;
//! * the **coupled** relation in [`relation_v1`] binds every fill to
//!   `(owner, outcome, side)`, closes per-outcome conservation through one
//!   global virtual split/merge pair, and proves from the witness alone that the
//!   accepted fills admit a complete executable pairing.
//! * the owner-blind economic core in [`relation_v2`] accepts only coefficient-
//!   vector orders and aggregate economic deltas. Owner, signer, account, fee,
//!   dealer, and settlement bindings are deliberately outside its type system.
//!
//! Both are IMPLEMENTED host-model code.  Neither is verified, and neither is
//! the SVM relation.

pub mod dealer_leg_v2;
pub mod direct_pair_v1;
pub mod portfolio_book_v2;
pub mod portfolio_execution_v2;
pub mod relation_v1;
pub mod relation_v1_stream;
pub mod relation_v1_stream_v2;
pub mod relation_v2;
pub mod relation_v2_ranking;
pub mod relation_v2_stream;

#[cfg(test)]
mod dealer_leg_v2_tests;
#[cfg(test)]
mod direct_pair_v1_tests;
#[cfg(test)]
mod portfolio_book_v2_tests;
#[cfg(test)]
mod portfolio_execution_v2_tests;
#[cfg(test)]
mod relation_v1_stream_v2_tests;
#[cfg(test)]
mod relation_v2_tests;
#[cfg(test)]
mod relation_v2_ranking_tests;
pub mod score_v2;

#[cfg(test)]
mod score_v2_tests;

pub const MAX_ORDERS: usize = 64;
pub const MAX_GRID_TICKS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartialPolicy {
    Allow,
    AllOrNone,
}

/// Remainder handling is part of the frozen relation, never a solver choice.
/// `Reject` is available for deployments that have not accepted a dust policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DustPolicy {
    /// Assign leftover atoms by largest remainder, then seeded canonical rank.
    AssignCanonical,
    /// Refuse any allocation that would require a leftover atom.
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TieRule {
    /// Maximize quantity, minimize absolute imbalance, then choose the highest
    /// grid tick.  This is the only tie rule implemented by V1.
    MaxVolumeMinImbalanceHighestTick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceGrid {
    pub ticks: [u64; MAX_GRID_TICKS],
    pub len: u8,
}

impl PriceGrid {
    pub fn new(ticks: [u64; MAX_GRID_TICKS], len: u8) -> Result<Self, Error> {
        let grid = Self { ticks, len };
        grid.validate()?;
        Ok(grid)
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.len == 0 || self.len as usize > MAX_GRID_TICKS {
            return Err(Error::InvalidGrid);
        }
        let mut i = 1usize;
        while i < self.len as usize {
            if self.ticks[i - 1] >= self.ticks[i] {
                return Err(Error::InvalidGrid);
            }
            i += 1;
        }
        // Non-canonical padding cannot influence a digest or a later fold.
        while i < MAX_GRID_TICKS {
            if self.ticks[i] != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            i += 1;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrozenPolicy {
    pub grid: PriceGrid,
    pub tie_rule: TieRule,
    pub dust_policy: DustPolicy,
    pub remainder_seed: u64,
}

impl FrozenPolicy {
    pub fn new(
        grid: PriceGrid,
        tie_rule: TieRule,
        dust_policy: DustPolicy,
        remainder_seed: u64,
    ) -> Result<Self, Error> {
        grid.validate()?;
        Ok(Self {
            grid,
            tie_rule,
            dust_policy,
            remainder_seed,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Order {
    /// The order identifier is the canonical tie-break identity.
    pub canonical_order_id: u64,
    pub side: Side,
    pub limit_tick: u8,
    pub quantity: u64,
    pub minimum_fill: u64,
    pub partial_policy: PartialPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedBook {
    pub policy: FrozenPolicy,
    pub orders: [Order; MAX_ORDERS],
    pub len: u8,
}

impl FixedBook {
    pub fn new(policy: FrozenPolicy, orders: [Order; MAX_ORDERS], len: u8) -> Result<Self, Error> {
        let book = Self {
            policy,
            orders,
            len,
        };
        book.validate()?;
        Ok(book)
    }

    pub fn validate(&self) -> Result<(), Error> {
        self.policy.grid.validate()?;
        if self.len as usize > MAX_ORDERS {
            return Err(Error::TooManyOrders);
        }
        let mut i = 0usize;
        let mut previous_id = 0u64;
        while i < self.len as usize {
            let order = self.orders[i];
            if order.canonical_order_id == 0 || order.canonical_order_id <= previous_id {
                return Err(Error::NonCanonicalOrderOrder);
            }
            if order.limit_tick as usize >= self.policy.grid.len as usize {
                return Err(Error::InvalidTick);
            }
            if order.quantity == 0 || order.minimum_fill > order.quantity {
                return Err(Error::InvalidQuantity);
            }
            if order.partial_policy == PartialPolicy::AllOrNone
                && order.minimum_fill != order.quantity
            {
                return Err(Error::InvalidMinimumFill);
            }
            previous_id = order.canonical_order_id;
            i += 1;
        }
        while i < MAX_ORDERS {
            if self.orders[i].canonical_order_id != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            i += 1;
        }
        Ok(())
    }

    /// Construct the canonical candidate.  This is a constructor, not an
    /// optimality oracle; verification below recomputes every aggregate.
    pub fn propose(&self) -> Result<Candidate, Error> {
        self.validate()?;
        let clearing = self.choose_tick()?;
        let buy_total = self.side_total(clearing, Side::Buy)?;
        let sell_total = self.side_total(clearing, Side::Sell)?;
        let matched = if buy_total < sell_total {
            buy_total
        } else {
            sell_total
        };
        let mut candidate = Candidate {
            clearing_tick: clearing,
            fills: [0u64; MAX_ORDERS],
            len: self.len,
            matched,
        };
        self.allocate_side(clearing, Side::Buy, matched, &mut candidate.fills)?;
        self.allocate_side(clearing, Side::Sell, matched, &mut candidate.fills)?;
        self.verify(&candidate)
    }

    /// Verify a candidate using only the frozen book and the candidate fields.
    /// A candidate's claimed `matched` quantity is never trusted as an input.
    pub fn verify(&self, candidate: &Candidate) -> Result<Candidate, Error> {
        self.validate()?;
        if candidate.len != self.len
            || candidate.clearing_tick as usize >= self.policy.grid.len as usize
        {
            return Err(Error::CandidateMismatch);
        }
        let buy_total = self.side_total(candidate.clearing_tick, Side::Buy)?;
        let sell_total = self.side_total(candidate.clearing_tick, Side::Sell)?;
        let expected = if buy_total < sell_total {
            buy_total
        } else {
            sell_total
        };
        if candidate.matched != expected {
            return Err(Error::CandidateMismatch);
        }
        let (buy_filled, sell_filled) = self.validate_fills(candidate)?;
        if buy_filled != expected || sell_filled != expected {
            return Err(Error::ConservationFailure);
        }
        if self.choose_tick()? != candidate.clearing_tick {
            return Err(Error::CandidateMismatch);
        }
        // Aggregate conservation is necessary but not sufficient: a forged
        // candidate can move atoms between otherwise valid orders while
        // preserving both side totals. Recompute the frozen allocation and
        // require byte-for-byte equality with the canonical fill vector.
        let mut canonical_fills = [0u64; MAX_ORDERS];
        self.allocate_side(
            candidate.clearing_tick,
            Side::Buy,
            expected,
            &mut canonical_fills,
        )?;
        self.allocate_side(
            candidate.clearing_tick,
            Side::Sell,
            expected,
            &mut canonical_fills,
        )?;
        if candidate.fills != canonical_fills {
            return Err(Error::CandidateMismatch);
        }
        Ok(*candidate)
    }

    fn choose_tick(&self) -> Result<u8, Error> {
        let mut tick = 0usize;
        let mut best_tick = 0usize;
        let mut best_volume = 0u64;
        let mut best_imbalance = u64::MAX;
        let mut initialized = false;
        while tick < self.policy.grid.len as usize {
            let buy = self.side_total(tick as u8, Side::Buy)?;
            let sell = self.side_total(tick as u8, Side::Sell)?;
            let volume = if buy < sell { buy } else { sell };
            let imbalance = buy.abs_diff(sell);
            let better = !initialized
                || volume > best_volume
                || (volume == best_volume && imbalance < best_imbalance)
                || (volume == best_volume && imbalance == best_imbalance && tick > best_tick);
            if better {
                initialized = true;
                best_tick = tick;
                best_volume = volume;
                best_imbalance = imbalance;
            }
            tick += 1;
        }
        if !initialized {
            return Err(Error::NoGridTick);
        }
        Ok(best_tick as u8)
    }

    fn side_total(&self, clearing: u8, side: Side) -> Result<u64, Error> {
        let mut total = 0u64;
        let mut i = 0usize;
        while i < self.len as usize {
            let order = self.orders[i];
            let eligible = match side {
                Side::Buy => order.side == Side::Buy && order.limit_tick >= clearing,
                Side::Sell => order.side == Side::Sell && order.limit_tick <= clearing,
            };
            if eligible {
                total = total
                    .checked_add(order.quantity)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            i += 1;
        }
        Ok(total)
    }

    fn allocate_side(
        &self,
        clearing: u8,
        side: Side,
        target: u64,
        fills: &mut [u64; MAX_ORDERS],
    ) -> Result<(), Error> {
        let total = self.side_total(clearing, side)?;
        if target > total {
            return Err(Error::ConservationFailure);
        }
        if total == 0 || target == 0 {
            return Ok(());
        }
        let mut remainders = [0u128; MAX_ORDERS];
        let mut assigned = [false; MAX_ORDERS];
        let mut floor_sum = 0u64;
        let mut i = 0usize;
        while i < self.len as usize {
            let order = self.orders[i];
            let eligible = match side {
                Side::Buy => order.side == Side::Buy && order.limit_tick >= clearing,
                Side::Sell => order.side == Side::Sell && order.limit_tick <= clearing,
            };
            if eligible {
                let product = (order.quantity as u128) * (target as u128);
                let quotient = product / (total as u128);
                let remainder = product % (total as u128);
                let q = u64::try_from(quotient).map_err(|_| Error::ArithmeticOverflow)?;
                fills[i] = q;
                floor_sum = floor_sum.checked_add(q).ok_or(Error::ArithmeticOverflow)?;
                remainders[i] = remainder;
            }
            i += 1;
        }
        let dust = target - floor_sum;
        if dust != 0 && self.policy.dust_policy == DustPolicy::Reject {
            return Err(Error::DustRejected);
        }
        let mut left = dust;
        while left != 0 {
            let mut selected: Option<usize> = None;
            let mut j = 0usize;
            while j < self.len as usize {
                let order = self.orders[j];
                let eligible = match side {
                    Side::Buy => order.side == Side::Buy && order.limit_tick >= clearing,
                    Side::Sell => order.side == Side::Sell && order.limit_tick <= clearing,
                };
                if eligible && !assigned[j] {
                    let is_better = match selected {
                        None => true,
                        Some(k) => {
                            remainders[j] > remainders[k]
                                || (remainders[j] == remainders[k]
                                    && seeded_rank(
                                        order.canonical_order_id,
                                        self.policy.remainder_seed,
                                    ) < seeded_rank(
                                        self.orders[k].canonical_order_id,
                                        self.policy.remainder_seed,
                                    ))
                                || (remainders[j] == remainders[k]
                                    && seeded_rank(
                                        order.canonical_order_id,
                                        self.policy.remainder_seed,
                                    ) == seeded_rank(
                                        self.orders[k].canonical_order_id,
                                        self.policy.remainder_seed,
                                    )
                                    && order.canonical_order_id < self.orders[k].canonical_order_id)
                        }
                    };
                    if is_better {
                        selected = Some(j);
                    }
                }
                j += 1;
            }
            let index = selected.ok_or(Error::ArithmeticOverflow)?;
            fills[index] = fills[index]
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
            assigned[index] = true;
            left -= 1;
        }
        Ok(())
    }

    fn validate_fills(&self, candidate: &Candidate) -> Result<(u64, u64), Error> {
        let mut buy = 0u64;
        let mut sell = 0u64;
        let mut i = 0usize;
        while i < self.len as usize {
            let order = self.orders[i];
            let fill = candidate.fills[i];
            let eligible = match order.side {
                Side::Buy => order.limit_tick >= candidate.clearing_tick,
                Side::Sell => order.limit_tick <= candidate.clearing_tick,
            };
            if fill != 0 && !eligible {
                return Err(Error::IneligibleFill);
            }
            if fill > order.quantity {
                return Err(Error::FillExceedsQuantity);
            }
            if fill != 0 && fill < order.minimum_fill {
                return Err(Error::MinimumFillViolation);
            }
            if order.partial_policy == PartialPolicy::AllOrNone
                && fill != 0
                && fill != order.quantity
            {
                return Err(Error::AllOrNoneViolation);
            }
            match order.side {
                Side::Buy => buy = buy.checked_add(fill).ok_or(Error::ArithmeticOverflow)?,
                Side::Sell => sell = sell.checked_add(fill).ok_or(Error::ArithmeticOverflow)?,
            }
            i += 1;
        }
        while i < MAX_ORDERS {
            if candidate.fills[i] != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            i += 1;
        }
        Ok((buy, sell))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub clearing_tick: u8,
    pub fills: [u64; MAX_ORDERS],
    pub len: u8,
    pub matched: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidGrid,
    InvalidTick,
    TooManyOrders,
    InvalidQuantity,
    InvalidMinimumFill,
    NonCanonicalOrderOrder,
    NonCanonicalPadding,
    NoGridTick,
    ArithmeticOverflow,
    CandidateMismatch,
    ConservationFailure,
    FillExceedsQuantity,
    IneligibleFill,
    MinimumFillViolation,
    AllOrNoneViolation,
    DustRejected,
}

pub(crate) fn seeded_rank(order_id: u64, seed: u64) -> u64 {
    // A fixed SplitMix-style permutation seeded by the frozen policy.
    let mut x = order_id ^ seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (x ^ (x >> 31)).rotate_left(17)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> PriceGrid {
        let mut ticks = [0u64; MAX_GRID_TICKS];
        ticks[0] = 10;
        ticks[1] = 20;
        ticks[2] = 30;
        PriceGrid::new(ticks, 3).unwrap()
    }

    fn policy(dust: DustPolicy) -> FrozenPolicy {
        FrozenPolicy::new(grid(), TieRule::MaxVolumeMinImbalanceHighestTick, dust, 7).unwrap()
    }

    fn empty(_policy: FrozenPolicy) -> [Order; MAX_ORDERS] {
        [Order {
            canonical_order_id: 0,
            side: Side::Buy,
            limit_tick: 0,
            quantity: 0,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
        }; MAX_ORDERS]
    }

    fn order(id: u64, side: Side, tick: u8, quantity: u64) -> Order {
        Order {
            canonical_order_id: id,
            side,
            limit_tick: tick,
            quantity,
            minimum_fill: 1,
            partial_policy: PartialPolicy::Allow,
        }
    }

    #[test]
    fn highest_tick_wins_equal_volume_and_imbalance() {
        let mut orders = empty(policy(DustPolicy::AssignCanonical));
        orders[0] = order(1, Side::Buy, 2, 10);
        orders[1] = order(2, Side::Buy, 1, 10);
        orders[2] = order(3, Side::Sell, 1, 10);
        orders[3] = order(4, Side::Sell, 2, 10);
        let book = FixedBook::new(policy(DustPolicy::AssignCanonical), orders, 4).unwrap();
        let candidate = book.propose().unwrap();
        assert_eq!(candidate.clearing_tick, 2);
        assert_eq!(candidate.matched, 10);
        assert_eq!(candidate.fills[0] + candidate.fills[1], 10);
        assert_eq!(candidate.fills[2] + candidate.fills[3], 10);
    }

    #[test]
    fn pro_rata_assigns_every_atom_and_is_repeatable() {
        let mut orders = empty(policy(DustPolicy::AssignCanonical));
        orders[0] = order(1, Side::Buy, 2, 10);
        orders[1] = order(2, Side::Buy, 2, 10);
        orders[2] = order(3, Side::Sell, 0, 7);
        let book = FixedBook::new(policy(DustPolicy::AssignCanonical), orders, 3).unwrap();
        let a = book.propose().unwrap();
        let b = book.propose().unwrap();
        assert_eq!(a, b);
        assert_eq!(a.fills[0] + a.fills[1], 7);
        assert_eq!(a.fills[2], 7);
    }

    #[test]
    fn unresolved_dust_can_be_refused() {
        let mut orders = empty(policy(DustPolicy::Reject));
        orders[0] = order(1, Side::Buy, 1, 2);
        orders[1] = order(2, Side::Buy, 1, 1);
        orders[2] = order(3, Side::Sell, 0, 2);
        let book = FixedBook::new(policy(DustPolicy::Reject), orders, 3).unwrap();
        assert_eq!(book.propose(), Err(Error::DustRejected));
    }

    #[test]
    fn forged_candidate_is_rejected_and_conservation_holds() {
        let mut orders = empty(policy(DustPolicy::AssignCanonical));
        orders[0] = order(1, Side::Buy, 1, 4);
        orders[1] = order(2, Side::Sell, 1, 3);
        let book = FixedBook::new(policy(DustPolicy::AssignCanonical), orders, 2).unwrap();
        let mut candidate = book.propose().unwrap();
        assert_eq!(candidate.matched, 3);
        candidate.fills[1] = 2;
        assert_eq!(book.verify(&candidate), Err(Error::ConservationFailure));
    }

    #[test]
    fn forged_pro_rata_reweighting_is_rejected() {
        let mut orders = empty(policy(DustPolicy::AssignCanonical));
        orders[0] = order(1, Side::Buy, 2, 10);
        orders[1] = order(2, Side::Buy, 2, 10);
        orders[2] = order(3, Side::Sell, 0, 10);
        let book = FixedBook::new(policy(DustPolicy::AssignCanonical), orders, 3).unwrap();
        let canonical = book.propose().unwrap();
        assert_eq!(canonical.fills[0], 5);
        assert_eq!(canonical.fills[1], 5);

        let forged = Candidate {
            clearing_tick: canonical.clearing_tick,
            fills: {
                let mut fills = canonical.fills;
                fills[0] = 10;
                fills[1] = 0;
                fills
            },
            len: canonical.len,
            matched: canonical.matched,
        };
        assert_eq!(book.verify(&forged), Err(Error::CandidateMismatch));
    }

    #[test]
    fn ineligible_order_cannot_receive_a_fill() {
        let mut orders = empty(policy(DustPolicy::AssignCanonical));
        orders[0] = order(1, Side::Buy, 2, 10);
        orders[1] = order(2, Side::Buy, 0, 10);
        orders[2] = order(3, Side::Sell, 2, 10);
        let book = FixedBook::new(policy(DustPolicy::AssignCanonical), orders, 3).unwrap();
        let canonical = book.propose().unwrap();
        assert_eq!(canonical.clearing_tick, 2);
        assert_eq!(canonical.fills[0], 10);
        assert_eq!(canonical.fills[1], 0);

        let forged = Candidate {
            clearing_tick: canonical.clearing_tick,
            fills: {
                let mut fills = canonical.fills;
                fills[0] = 0;
                fills[1] = 10;
                fills
            },
            len: canonical.len,
            matched: canonical.matched,
        };
        assert_eq!(book.verify(&forged), Err(Error::IneligibleFill));
    }

    #[test]
    fn all_or_none_cannot_bypass_canonical_allocation() {
        let mut orders = empty(policy(DustPolicy::AssignCanonical));
        orders[0] = Order {
            canonical_order_id: 1,
            side: Side::Buy,
            limit_tick: 2,
            quantity: 10,
            minimum_fill: 10,
            partial_policy: PartialPolicy::AllOrNone,
        };
        orders[1] = order(2, Side::Buy, 2, 10);
        orders[2] = order(3, Side::Sell, 0, 15);
        let book = FixedBook::new(policy(DustPolicy::AssignCanonical), orders, 3).unwrap();
        // The frozen pro-rata allocator cannot make the AON order's marginal
        // quantity whole, so construction refuses the candidate.
        assert!(book.propose().is_err());

        // A manually supplied AON fill is not a license to bypass that refusal.
        let forged = Candidate {
            clearing_tick: 2,
            fills: {
                let mut fills = [0u64; MAX_ORDERS];
                fills[0] = 10;
                fills[1] = 5;
                fills[2] = 15;
                fills
            },
            len: 3,
            matched: 15,
        };
        assert_eq!(book.verify(&forged), Err(Error::CandidateMismatch));
    }

    #[test]
    fn canonical_order_sequence_is_required() {
        let mut orders = empty(policy(DustPolicy::AssignCanonical));
        orders[0] = order(2, Side::Buy, 1, 1);
        orders[1] = order(1, Side::Sell, 0, 1);
        assert_eq!(
            FixedBook::new(policy(DustPolicy::AssignCanonical), orders, 2),
            Err(Error::NonCanonicalOrderOrder)
        );
    }

    #[test]
    fn batch_rejects_buy_below_clearing_tick() {
        // The §6 name for the repaired ineligible-fill gate, buy direction: a
        // buy whose limit is below the clearing tick may never carry a fill.
        let mut orders = empty(policy(DustPolicy::AssignCanonical));
        orders[0] = order(1, Side::Buy, 2, 10);
        orders[1] = order(2, Side::Buy, 0, 10);
        orders[2] = order(3, Side::Sell, 2, 10);
        let book = FixedBook::new(policy(DustPolicy::AssignCanonical), orders, 3).unwrap();
        let canonical = book.propose().unwrap();
        assert_eq!(canonical.clearing_tick, 2);
        let forged = Candidate {
            clearing_tick: 2,
            fills: {
                let mut fills = [0u64; MAX_ORDERS];
                fills[1] = 10;
                fills[2] = 10;
                fills
            },
            len: 3,
            matched: canonical.matched,
        };
        assert_eq!(book.verify(&forged), Err(Error::IneligibleFill));
    }

    #[test]
    fn batch_rejects_sell_above_clearing_tick() {
        // The inverse inequality: a sell whose limit is above the clearing tick
        // may never carry a fill either.  Both comparisons are gated.
        let mut orders = empty(policy(DustPolicy::AssignCanonical));
        orders[0] = order(1, Side::Buy, 0, 10);
        orders[1] = order(2, Side::Sell, 0, 10);
        orders[2] = order(3, Side::Sell, 2, 10);
        let book = FixedBook::new(policy(DustPolicy::AssignCanonical), orders, 3).unwrap();
        let canonical = book.propose().unwrap();
        assert_eq!(canonical.clearing_tick, 0);
        assert_eq!(canonical.fills[1], 10);
        assert_eq!(canonical.fills[2], 0);
        let forged = Candidate {
            clearing_tick: 0,
            fills: {
                let mut fills = [0u64; MAX_ORDERS];
                fills[0] = 10;
                fills[2] = 10;
                fills
            },
            len: 3,
            matched: canonical.matched,
        };
        assert_eq!(book.verify(&forged), Err(Error::IneligibleFill));
    }

    #[test]
    fn bounded_conservation_property_for_small_books() {
        let mut buy_quantity = 1u64;
        while buy_quantity <= 8 {
            let mut sell_quantity = 1u64;
            while sell_quantity <= 8 {
                let mut orders = empty(policy(DustPolicy::AssignCanonical));
                orders[0] = order(1, Side::Buy, 2, buy_quantity);
                orders[1] = order(2, Side::Buy, 1, 3);
                orders[2] = order(3, Side::Sell, 0, sell_quantity);
                orders[3] = order(4, Side::Sell, 1, 2);
                let book = FixedBook::new(policy(DustPolicy::AssignCanonical), orders, 4).unwrap();
                let candidate = book.propose().unwrap();
                let buys = candidate.fills[0] + candidate.fills[1];
                let sells = candidate.fills[2] + candidate.fills[3];
                assert_eq!(buys, candidate.matched);
                assert_eq!(sells, candidate.matched);
                assert!(candidate.fills[0] <= buy_quantity);
                assert!(candidate.fills[1] <= 3);
                assert!(candidate.fills[2] <= sell_quantity);
                assert!(candidate.fills[3] <= 2);
                sell_quantity += 1;
            }
            buy_quantity += 1;
        }
    }
}
