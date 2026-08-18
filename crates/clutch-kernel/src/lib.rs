#![no_std]
#![forbid(unsafe_code)]

//! A bounded, collateral-generic complete-claim kernel.
//!
//! This crate owns only pure semantic transitions.  It has no token, account,
//! clock, hashing, serialization, or Solana dependencies.  `MarketState` keeps
//! a conservative aggregate claim supply; a caller supplies a `Position` for
//! the particular owner being transitioned.  An adapter must reconcile that
//! aggregate with its authenticated account state before invoking a transition.
//!
//! The kernel intentionally has no discretionary resolver.  Resolution accepts
//! only an index into the immutable finite payout set.  It does not claim that
//! a source, adapter, or external observation is authentic.
//!
//! `Amount` is an opaque collateral-atom quantity.  No collateral mint,
//! decimal system, or asset-specific rule is embedded in this crate.

use core::convert::TryFrom;

pub const MAX_OUTCOMES: usize = 16;
pub const MAX_PAYOUTS: usize = 8;
pub const MIN_OUTCOMES: u8 = 2;

pub type Amount = u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    InvalidOutcomeCount,
    InvalidPayoutCount,
    InvalidPayoutIndex,
    InvalidDenominator,
    InvalidPayoutWeights,
    ZeroQuantity,
    ArithmeticOverflow,
    ArithmeticUnderflow,
    InsufficientBalance,
    InsufficientCollateral,
    NotActive,
    AlreadyResolved,
    NotResolved,
    InvariantViolation,
    RemainderRequired,
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PayoutVector {
    pub denominator: u64,
    pub weights: [u64; MAX_OUTCOMES],
}

impl PayoutVector {
    pub const ZERO: Self = Self {
        denominator: 0,
        weights: [0; MAX_OUTCOMES],
    };

    pub const fn new(denominator: u64, weights: [u64; MAX_OUTCOMES]) -> Self {
        Self {
            denominator,
            weights,
        }
    }

    fn validate(&self, outcome_count: u8) -> Result<()> {
        if self.denominator == 0 {
            return Err(Error::InvalidDenominator);
        }
        let count = usize::from(outcome_count);
        let mut sum = 0_u64;
        let mut i = 0_usize;
        while i < MAX_OUTCOMES {
            let weight = self.weights[i];
            if i >= count {
                if weight != 0 {
                    return Err(Error::InvalidPayoutWeights);
                }
            } else {
                if weight > self.denominator {
                    return Err(Error::InvalidPayoutWeights);
                }
                sum = sum.checked_add(weight).ok_or(Error::ArithmeticOverflow)?;
            }
            i += 1;
        }
        if sum != self.denominator {
            return Err(Error::InvalidPayoutWeights);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PayoutSet {
    pub count: u8,
    pub outcomes: u8,
    pub vectors: [PayoutVector; MAX_PAYOUTS],
}

impl PayoutSet {
    pub const EMPTY: Self = Self {
        count: 0,
        outcomes: 0,
        vectors: [PayoutVector::ZERO; MAX_PAYOUTS],
    };

    pub const fn new(count: u8, outcomes: u8, vectors: [PayoutVector; MAX_PAYOUTS]) -> Self {
        Self {
            count,
            outcomes,
            vectors,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.outcomes < MIN_OUTCOMES || usize::from(self.outcomes) > MAX_OUTCOMES {
            return Err(Error::InvalidOutcomeCount);
        }
        if self.count == 0 || usize::from(self.count) > MAX_PAYOUTS {
            return Err(Error::InvalidPayoutCount);
        }
        let common_denominator = self.vectors[0].denominator;
        let mut i = 0_usize;
        while i < MAX_PAYOUTS {
            if i < usize::from(self.count) {
                self.vectors[i].validate(self.outcomes)?;
                if self.vectors[i].denominator != common_denominator {
                    return Err(Error::InvalidDenominator);
                }
            } else if self.vectors[i] != PayoutVector::ZERO {
                return Err(Error::InvalidPayoutWeights);
            }
            i += 1;
        }
        Ok(())
    }

    fn get(&self, index: u8) -> Result<&PayoutVector> {
        if index >= self.count {
            return Err(Error::InvalidPayoutIndex);
        }
        let index = usize::from(index);
        Ok(&self.vectors[index])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Phase {
    Active = 0,
    Resolved = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct MarketState {
    pub outcomes: u8,
    pub phase: Phase,
    /// Index in `payouts` after resolution; meaningless while Active.
    pub resolved_payout: u8,
    pub collateral: Amount,
    /// Total internal plus conservatively accounted external claims per outcome.
    pub total_supply: [Amount; MAX_OUTCOMES],
    pub payouts: PayoutSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Position {
    pub internal: [Amount; MAX_OUTCOMES],
    pub external: [Amount; MAX_OUTCOMES],
}

impl Position {
    pub const EMPTY: Self = Self {
        internal: [0; MAX_OUTCOMES],
        external: [0; MAX_OUTCOMES],
    };

    fn validate_outcome(outcomes: u8, outcome: u8) -> Result<usize> {
        if outcome >= outcomes {
            return Err(Error::InvalidPayoutIndex);
        }
        Ok(usize::from(outcome))
    }
}

impl MarketState {
    pub fn new(outcomes: u8, payouts: PayoutSet, collateral: Amount) -> Result<Self> {
        if outcomes != payouts.outcomes {
            return Err(Error::InvalidOutcomeCount);
        }
        payouts.validate()?;
        let state = Self {
            outcomes,
            phase: Phase::Active,
            resolved_payout: 0,
            collateral,
            total_supply: [0; MAX_OUTCOMES],
            payouts,
        };
        state.check_invariants()?;
        Ok(state)
    }

    pub fn required_collateral(&self) -> Result<Amount> {
        self.validate_shape()?;
        match self.phase {
            Phase::Active => {
                let mut max = 0_u64;
                let mut j = 0_usize;
                while j < usize::from(self.payouts.count) {
                    let value = self.required_for_vector(&self.payouts.vectors[j])?;
                    if value > max {
                        max = value;
                    }
                    j += 1;
                }
                Ok(max)
            }
            Phase::Resolved => self.required_for_vector(self.payouts.get(self.resolved_payout)?),
        }
    }

    fn required_for_vector(&self, vector: &PayoutVector) -> Result<Amount> {
        let mut numerator = 0_u128;
        let mut i = 0_usize;
        while i < usize::from(self.outcomes) {
            let term = u128::from(self.total_supply[i])
                .checked_mul(u128::from(vector.weights[i]))
                .ok_or(Error::ArithmeticOverflow)?;
            numerator = numerator
                .checked_add(term)
                .ok_or(Error::ArithmeticOverflow)?;
            i += 1;
        }
        let denominator = u128::from(vector.denominator);
        let quotient = numerator / denominator;
        let remainder = numerator % denominator;
        let rounded = if remainder == 0 {
            quotient
        } else {
            quotient.checked_add(1).ok_or(Error::ArithmeticOverflow)?
        };
        Amount::try_from(rounded).map_err(|_| Error::ArithmeticOverflow)
    }

    pub fn check_invariants(&self) -> Result<()> {
        self.validate_shape()
            .map_err(|_| Error::InvariantViolation)?;
        if self.phase == Phase::Resolved && self.resolved_payout >= self.payouts.count {
            return Err(Error::InvariantViolation);
        }
        let required = self.required_collateral()?;
        if self.collateral < required {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<()> {
        if self.outcomes != self.payouts.outcomes {
            return Err(Error::InvalidOutcomeCount);
        }
        self.payouts.validate()
    }

    pub fn split(&mut self, position: &mut Position, quantity: Amount) -> Result<()> {
        self.validate_shape()?;
        self.check_invariants()?;
        self.require_active()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let new_collateral = self
            .collateral
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        let count = usize::from(self.outcomes);
        let mut i = 0_usize;
        while i < count {
            self.total_supply[i]
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            position.internal[i]
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            i += 1;
        }
        self.collateral = new_collateral;
        let mut i = 0_usize;
        while i < count {
            self.total_supply[i] = self.total_supply[i]
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            position.internal[i] = position.internal[i]
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            i += 1;
        }
        self.check_invariants()
    }

    pub fn merge(&mut self, position: &mut Position, quantity: Amount) -> Result<()> {
        self.validate_shape()?;
        self.check_invariants()?;
        self.require_active()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        if self.collateral < quantity {
            return Err(Error::InsufficientCollateral);
        }
        let count = usize::from(self.outcomes);
        let mut i = 0_usize;
        while i < count {
            if position.internal[i] < quantity || self.total_supply[i] < quantity {
                return Err(Error::InsufficientBalance);
            }
            i += 1;
        }
        self.collateral = self
            .collateral
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        let mut i = 0_usize;
        while i < count {
            position.internal[i] = position.internal[i]
                .checked_sub(quantity)
                .ok_or(Error::ArithmeticUnderflow)?;
            self.total_supply[i] = self.total_supply[i]
                .checked_sub(quantity)
                .ok_or(Error::ArithmeticUnderflow)?;
            i += 1;
        }
        self.check_invariants()
    }

    pub fn materialize(
        &mut self,
        position: &mut Position,
        outcome: u8,
        quantity: Amount,
    ) -> Result<()> {
        self.validate_shape()?;
        self.check_invariants()?;
        self.require_active()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let i = Position::validate_outcome(self.outcomes, outcome)?;
        if position.internal[i] < quantity {
            return Err(Error::InsufficientBalance);
        }
        position.external[i] = position.external[i]
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        position.internal[i] = position.internal[i]
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        // Total claim supply and collateral do not change at this boundary.
        self.check_invariants()
    }

    pub fn dematerialize(
        &mut self,
        position: &mut Position,
        outcome: u8,
        quantity: Amount,
    ) -> Result<()> {
        self.validate_shape()?;
        self.check_invariants()?;
        self.require_active()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let i = Position::validate_outcome(self.outcomes, outcome)?;
        if position.external[i] < quantity {
            return Err(Error::InsufficientBalance);
        }
        position.internal[i] = position.internal[i]
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        position.external[i] = position.external[i]
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        self.check_invariants()
    }

    pub fn resolve(&mut self, payout_index: u8) -> Result<()> {
        self.validate_shape()?;
        self.check_invariants()?;
        self.require_active()?;
        self.payouts.get(payout_index)?;
        self.phase = Phase::Resolved;
        self.resolved_payout = payout_index;
        self.check_invariants()
    }

    pub fn redeem_internal(
        &mut self,
        position: &mut Position,
        outcome: u8,
        quantity: Amount,
    ) -> Result<Amount> {
        self.redeem(position, outcome, quantity, true)
    }

    pub fn redeem_external(
        &mut self,
        position: &mut Position,
        outcome: u8,
        quantity: Amount,
    ) -> Result<Amount> {
        self.redeem(position, outcome, quantity, false)
    }

    fn redeem(
        &mut self,
        position: &mut Position,
        outcome: u8,
        quantity: Amount,
        internal: bool,
    ) -> Result<Amount> {
        self.validate_shape()?;
        if self.phase != Phase::Resolved {
            return Err(Error::NotResolved);
        }
        self.check_invariants()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let i = Position::validate_outcome(self.outcomes, outcome)?;
        let available = if internal {
            position.internal[i]
        } else {
            position.external[i]
        };
        if available < quantity || self.total_supply[i] < quantity {
            return Err(Error::InsufficientBalance);
        }
        let vector = *self.payouts.get(self.resolved_payout)?;
        let numerator = u128::from(quantity)
            .checked_mul(u128::from(vector.weights[i]))
            .ok_or(Error::ArithmeticOverflow)?;
        let denominator = u128::from(vector.denominator);
        if numerator % denominator != 0 {
            return Err(Error::RemainderRequired);
        }
        let payout =
            Amount::try_from(numerator / denominator).map_err(|_| Error::ArithmeticOverflow)?;
        if self.collateral < payout {
            return Err(Error::InsufficientCollateral);
        }
        if internal {
            position.internal[i] = position.internal[i]
                .checked_sub(quantity)
                .ok_or(Error::ArithmeticUnderflow)?;
        } else {
            position.external[i] = position.external[i]
                .checked_sub(quantity)
                .ok_or(Error::ArithmeticUnderflow)?;
        }
        self.total_supply[i] = self.total_supply[i]
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        self.collateral = self
            .collateral
            .checked_sub(payout)
            .ok_or(Error::ArithmeticUnderflow)?;
        self.check_invariants()?;
        Ok(payout)
    }

    fn require_active(&self) -> Result<()> {
        if self.phase != Phase::Active {
            Err(Error::AlreadyResolved)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_hot(_outcome_count: u8, index: usize) -> PayoutVector {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[index] = 1;
        PayoutVector::new(1, weights)
    }

    fn binary_set() -> PayoutSet {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        vectors[0] = one_hot(2, 0);
        vectors[1] = one_hot(2, 1);
        PayoutSet::new(2, 2, vectors)
    }

    #[test]
    fn complete_split_merge_preserves_claims_and_collateral() {
        let mut market = MarketState::new(2, binary_set(), 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 17).unwrap();
        assert_eq!(market.collateral, 17);
        assert_eq!(market.total_supply[0], 17);
        assert_eq!(market.total_supply[1], 17);
        assert_eq!(position.internal[0], 17);
        market.merge(&mut position, 9).unwrap();
        assert_eq!(market.collateral, 8);
        assert_eq!(market.total_supply[0], 8);
        assert_eq!(position.internal[1], 8);
        assert_eq!(market.required_collateral().unwrap(), 8);
    }

    #[test]
    fn materialization_is_supply_neutral_and_round_trips() {
        let mut market = MarketState::new(2, binary_set(), 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 5).unwrap();
        market.materialize(&mut position, 1, 3).unwrap();
        assert_eq!(position.internal[1], 2);
        assert_eq!(position.external[1], 3);
        assert_eq!(market.total_supply[1], 5);
        market.dematerialize(&mut position, 1, 3).unwrap();
        assert_eq!(position.internal[1], 5);
        assert_eq!(position.external[1], 0);
    }

    #[test]
    fn resolution_is_finite_and_redemption_is_exact() {
        let mut market = MarketState::new(2, binary_set(), 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 11).unwrap();
        market.resolve(1).unwrap();
        assert_eq!(market.redeem_internal(&mut position, 0, 1), Ok(0));
        assert_eq!(market.redeem_internal(&mut position, 1, 10), Ok(10));
        assert_eq!(market.collateral, 1);
        assert_eq!(market.redeem_internal(&mut position, 1, 1), Ok(1));
        assert_eq!(market.collateral, 0);
        assert_eq!(
            market.redeem_internal(&mut position, 1, 1),
            Err(Error::InsufficientBalance)
        );
    }

    #[test]
    fn weighted_payout_refuses_unrepresentable_remainder() {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = 1;
        weights[1] = 1;
        vectors[0] = PayoutVector::new(2, weights);
        let set = PayoutSet::new(1, 2, vectors);
        let mut market = MarketState::new(2, set, 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 1).unwrap();
        market.resolve(0).unwrap();
        assert_eq!(
            market.redeem_internal(&mut position, 0, 1),
            Err(Error::RemainderRequired)
        );
    }

    #[test]
    fn malformed_payouts_and_overflow_are_refused() {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = 2;
        vectors[0] = PayoutVector::new(1, weights);
        assert_eq!(
            MarketState::new(2, PayoutSet::new(1, 2, vectors), 0),
            Err(Error::InvalidPayoutWeights)
        );

        let mut market = MarketState::new(2, binary_set(), 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, Amount::MAX).unwrap();
        assert_eq!(
            market.split(&mut position, 1),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(market.collateral, Amount::MAX);
    }

    #[test]
    fn public_operations_refuse_mutated_invalid_state() {
        let mut market = MarketState::new(2, binary_set(), 0).unwrap();
        market.payouts.vectors[0].denominator = 0;
        let mut position = Position::EMPTY;
        assert_eq!(market.required_collateral(), Err(Error::InvalidDenominator));
        assert_eq!(
            market.split(&mut position, 1),
            Err(Error::InvalidDenominator)
        );
        assert_eq!(market.resolve(0), Err(Error::InvalidDenominator));
    }

    #[test]
    fn repeated_small_traces_preserve_invariant() {
        let mut market = MarketState::new(
            3,
            {
                let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
                vectors[0] = one_hot(3, 0);
                vectors[1] = one_hot(3, 1);
                vectors[2] = one_hot(3, 2);
                PayoutSet::new(3, 3, vectors)
            },
            0,
        )
        .unwrap();
        let mut position = Position::EMPTY;
        let mut n = 1_u64;
        while n < 50 {
            market.split(&mut position, n).unwrap();
            market.check_invariants().unwrap();
            market.merge(&mut position, n).unwrap();
            market.check_invariants().unwrap();
            n += 1;
        }
        assert_eq!(
            market,
            MarketState::new(
                3,
                {
                    let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
                    vectors[0] = one_hot(3, 0);
                    vectors[1] = one_hot(3, 1);
                    vectors[2] = one_hot(3, 2);
                    PayoutSet::new(3, 3, vectors)
                },
                0
            )
            .unwrap()
        );
    }
}
