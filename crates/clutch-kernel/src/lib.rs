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

/// Phase gate for [`MarketState::transfer_internal`].
///
/// The two variants are the frozen alternatives T-a and T-b of
/// `docs/implementation/BATCH_RELATION_V1_DESIGN.md` §14.2.  Neither is a
/// default: a caller names one at every call site, so the choice stays a
/// reviewable policy rather than an implicit kernel opinion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TransferPhasePolicy {
    /// T-a: claims move only while the market is Active.  Settlement that races
    /// resolution is refused with `Error::AlreadyResolved`, which needs an
    /// epoch/resolution ordering rule outside the kernel.
    ActiveOnly = 0,
    /// T-b: claims move in either phase.  A transfer touches neither supply nor
    /// collateral, so every kernel invariant is phase-independent here and lazy
    /// settlement cannot be bricked by resolution.
    ActiveOrResolved = 1,
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
        self.required_collateral_for(&self.total_supply, self.phase, self.resolved_payout)
    }

    /// `required_collateral` evaluated against a prospective supply and
    /// resolution without materializing that state.  Transitions use it to
    /// judge their own result before they write it.
    fn required_collateral_for(
        &self,
        total_supply: &[Amount; MAX_OUTCOMES],
        phase: Phase,
        resolved_payout: u8,
    ) -> Result<Amount> {
        self.validate_shape()?;
        match phase {
            Phase::Active => {
                let mut max = 0_u64;
                let mut j = 0_usize;
                while j < usize::from(self.payouts.count) {
                    let value = self.required_for_vector(total_supply, &self.payouts.vectors[j])?;
                    if value > max {
                        max = value;
                    }
                    j += 1;
                }
                Ok(max)
            }
            Phase::Resolved => {
                self.required_for_vector(total_supply, self.payouts.get(resolved_payout)?)
            }
        }
    }

    fn required_for_vector(
        &self,
        total_supply: &[Amount; MAX_OUTCOMES],
        vector: &PayoutVector,
    ) -> Result<Amount> {
        let mut numerator = 0_u128;
        let mut i = 0_usize;
        while i < usize::from(self.outcomes) {
            let term = u128::from(total_supply[i])
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
        self.check_invariants_for(
            self.collateral,
            &self.total_supply,
            self.phase,
            self.resolved_payout,
        )
    }

    /// `check_invariants` for a prospective state that differs from `self` only
    /// in the fields a transition is about to write.  Every transition runs
    /// this check before its first write, which is what makes a refused
    /// transition leave the market and the caller's position untouched.
    fn check_invariants_for(
        &self,
        collateral: Amount,
        total_supply: &[Amount; MAX_OUTCOMES],
        phase: Phase,
        resolved_payout: u8,
    ) -> Result<()> {
        self.validate_shape()
            .map_err(|_| Error::InvariantViolation)?;
        if phase == Phase::Resolved && resolved_payout >= self.payouts.count {
            return Err(Error::InvariantViolation);
        }
        let required = self.required_collateral_for(total_supply, phase, resolved_payout)?;
        if collateral < required {
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

    /// Mint `quantity` internal claims of every outcome against `quantity`
    /// collateral atoms.
    ///
    /// Every check and every checked operation completes before the first
    /// write, including the invariant check over the prospective state: on
    /// `Err`, `self` and `position` are unchanged.
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
        let mut next_supply = self.total_supply;
        let mut next_internal = position.internal;
        let mut i = 0_usize;
        while i < count {
            next_supply[i] = next_supply[i]
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            next_internal[i] = next_internal[i]
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            i += 1;
        }
        self.check_invariants_for(
            new_collateral,
            &next_supply,
            self.phase,
            self.resolved_payout,
        )?;
        self.collateral = new_collateral;
        self.total_supply = next_supply;
        position.internal = next_internal;
        Ok(())
    }

    /// Burn `quantity` internal claims of every outcome and release `quantity`
    /// collateral atoms.
    ///
    /// The collateral test deliberately precedes the per-outcome balance tests.
    /// Because weights sum to the denominator, any state that passes the
    /// balance tests already holds `collateral >= quantity`, so this order is
    /// the only reason `Error::InsufficientCollateral` is observable from
    /// `merge` at all (`docs/implementation/VECTOR_SPINE_PROPOSAL.md` R8).
    /// Reordering would silently relabel a whole family of inputs, so the order
    /// is preserved verbatim; whether it is the intended order is a review
    /// question, not a refactor.
    ///
    /// Every check and every checked operation completes before the first
    /// write: on `Err`, `self` and `position` are unchanged.
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
        let new_collateral = self
            .collateral
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        let mut next_supply = self.total_supply;
        let mut next_internal = position.internal;
        let mut i = 0_usize;
        while i < count {
            next_internal[i] = next_internal[i]
                .checked_sub(quantity)
                .ok_or(Error::ArithmeticUnderflow)?;
            next_supply[i] = next_supply[i]
                .checked_sub(quantity)
                .ok_or(Error::ArithmeticUnderflow)?;
            i += 1;
        }
        self.check_invariants_for(
            new_collateral,
            &next_supply,
            self.phase,
            self.resolved_payout,
        )?;
        self.collateral = new_collateral;
        self.total_supply = next_supply;
        position.internal = next_internal;
        Ok(())
    }

    /// Move `quantity` of one outcome from the internal side of a position to
    /// its external (bearer) side.
    ///
    /// Every check and every checked operation completes before the first
    /// write: on `Err`, `self` and `position` are unchanged.
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
        let new_external = position.external[i]
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        let new_internal = position.internal[i]
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        // Total claim supply and collateral do not change at this boundary, so
        // the market invariant is re-checked over unchanged market fields.
        self.check_invariants()?;
        position.external[i] = new_external;
        position.internal[i] = new_internal;
        Ok(())
    }

    /// Move `quantity` of one outcome from the external (bearer) side of a
    /// position back to its internal side.
    ///
    /// Every check and every checked operation completes before the first
    /// write: on `Err`, `self` and `position` are unchanged.
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
        let new_internal = position.internal[i]
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        let new_external = position.external[i]
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        // Supply-neutral, exactly as in `materialize`.
        self.check_invariants()?;
        position.internal[i] = new_internal;
        position.external[i] = new_external;
        Ok(())
    }

    /// Fix the payout vector by index into the immutable finite payout set.
    ///
    /// Every check completes before the first write, the prospective invariant
    /// check included: on `Err`, `self` is unchanged.
    pub fn resolve(&mut self, payout_index: u8) -> Result<()> {
        self.validate_shape()?;
        self.check_invariants()?;
        self.require_active()?;
        self.payouts.get(payout_index)?;
        self.check_invariants_for(
            self.collateral,
            &self.total_supply,
            Phase::Resolved,
            payout_index,
        )?;
        self.phase = Phase::Resolved;
        self.resolved_payout = payout_index;
        Ok(())
    }

    /// Redeem `quantity` internal claims of one outcome after resolution and
    /// return the collateral paid.
    ///
    /// A payout that is not an exact number of atoms is refused with
    /// `Error::RemainderRequired` rather than floored.  For a fractional weight
    /// that refusal can be permanent for a single outcome; a balanced holder
    /// always exits through [`MarketState::redeem_complete_set`].
    ///
    /// Every check and every checked operation completes before the first
    /// write: on `Err`, `self` and `position` are unchanged.
    pub fn redeem_internal(
        &mut self,
        position: &mut Position,
        outcome: u8,
        quantity: Amount,
    ) -> Result<Amount> {
        self.redeem(position, outcome, quantity, true)
    }

    /// Redeem `quantity` external (bearer) claims of one outcome after
    /// resolution and return the collateral paid.
    ///
    /// Same exactness and transactionality contract as
    /// [`MarketState::redeem_internal`].
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
        let new_balance = available
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        let mut next_supply = self.total_supply;
        next_supply[i] = next_supply[i]
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        let new_collateral = self
            .collateral
            .checked_sub(payout)
            .ok_or(Error::ArithmeticUnderflow)?;
        self.check_invariants_for(
            new_collateral,
            &next_supply,
            self.phase,
            self.resolved_payout,
        )?;
        if internal {
            position.internal[i] = new_balance;
        } else {
            position.external[i] = new_balance;
        }
        self.total_supply = next_supply;
        self.collateral = new_collateral;
        Ok(payout)
    }

    /// Redeem `quantity` complete sets after resolution: burn `quantity`
    /// internal claims of every active outcome and pay exactly the per-set
    /// collateral.  Returns the collateral paid, which is always `quantity`.
    ///
    /// This is the Resolved-phase twin of [`MarketState::merge`] and the
    /// unconditional exit from the fractional-payout trap of
    /// `docs/implementation/POLICY_ANALYSIS_LOTS_FEES.md` §1.5.  A complete set
    /// never remainders: `sum_i q * w_i = q * D` for every admitted payout
    /// vector, so a holder of one unit of every outcome is never exit-dead even
    /// where each single-outcome redemption refuses forever.  An *unbalanced*
    /// fractional holding is not rescued by this transition and still depends
    /// on the unfrozen divisibility policy of §1.
    ///
    /// Only internal claims are redeemed, as in `merge`; external claims are
    /// the token adapter's seam and have no complete-set form here.
    ///
    /// Unlike `merge`, the per-outcome balance tests precede the collateral
    /// test, so `Error::InsufficientCollateral` is unreachable defense in depth
    /// on this path rather than an observable refusal.  The divergence is
    /// deliberate: `merge`'s order is pinned by R8 as landed behavior, while a
    /// new transition is free to report the balance fault that a caller can act
    /// on.
    ///
    /// Every check and every checked operation completes before the first
    /// write: on `Err`, `self` and `position` are unchanged.
    pub fn redeem_complete_set(
        &mut self,
        position: &mut Position,
        quantity: Amount,
    ) -> Result<Amount> {
        self.validate_shape()?;
        if self.phase != Phase::Resolved {
            return Err(Error::NotResolved);
        }
        self.check_invariants()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let count = usize::from(self.outcomes);
        let mut i = 0_usize;
        while i < count {
            if position.internal[i] < quantity || self.total_supply[i] < quantity {
                return Err(Error::InsufficientBalance);
            }
            i += 1;
        }
        let vector = *self.payouts.get(self.resolved_payout)?;
        let mut numerator = 0_u128;
        let mut i = 0_usize;
        while i < count {
            let term = u128::from(quantity)
                .checked_mul(u128::from(vector.weights[i]))
                .ok_or(Error::ArithmeticOverflow)?;
            numerator = numerator
                .checked_add(term)
                .ok_or(Error::ArithmeticOverflow)?;
            i += 1;
        }
        let denominator = u128::from(vector.denominator);
        let remainder = numerator % denominator;
        if remainder != 0 {
            // Unreachable while the payout set validates: the active weights
            // sum to the denominator.  Kept as the same refusal the
            // per-outcome path uses rather than a silent floor.
            return Err(Error::RemainderRequired);
        }
        let payout =
            Amount::try_from(numerator / denominator).map_err(|_| Error::ArithmeticOverflow)?;
        if payout != quantity {
            // The complete-set identity failed; the payout set is not the one
            // `validate_shape` admitted.
            return Err(Error::InvariantViolation);
        }
        if self.collateral < payout {
            return Err(Error::InsufficientCollateral);
        }
        let mut next_supply = self.total_supply;
        let mut next_internal = position.internal;
        let mut i = 0_usize;
        while i < count {
            next_internal[i] = next_internal[i]
                .checked_sub(quantity)
                .ok_or(Error::ArithmeticUnderflow)?;
            next_supply[i] = next_supply[i]
                .checked_sub(quantity)
                .ok_or(Error::ArithmeticUnderflow)?;
            i += 1;
        }
        let new_collateral = self
            .collateral
            .checked_sub(payout)
            .ok_or(Error::ArithmeticUnderflow)?;
        self.check_invariants_for(
            new_collateral,
            &next_supply,
            self.phase,
            self.resolved_payout,
        )?;
        self.collateral = new_collateral;
        self.total_supply = next_supply;
        position.internal = next_internal;
        Ok(payout)
    }

    /// Move `quantity` of one outcome's internal claims from one position to
    /// another.
    ///
    /// Taking `&self` makes supply and collateral neutrality structural: a
    /// transfer is not a market transition at all, only a relabelling of who
    /// holds an already-issued claim.  `phase_policy` is a required argument
    /// because T-a and T-b of
    /// `docs/implementation/BATCH_RELATION_V1_DESIGN.md` §14.2 are both live
    /// variants; the kernel holds no default opinion and the design's signature
    /// is extended by exactly this argument.
    ///
    /// Rust's borrow rules already forbid passing one `Position` as both `from`
    /// and `to`, so a self-transfer cannot be expressed.  Distinct *semantic*
    /// owners remain the caller's obligation: the kernel carries no owner
    /// identity and cannot check it.
    ///
    /// Every check and every checked operation completes before the first
    /// write: on `Err`, `from` and `to` are unchanged, and `self` is never
    /// written on any path.
    pub fn transfer_internal(
        &self,
        from: &mut Position,
        to: &mut Position,
        outcome: u8,
        quantity: Amount,
        phase_policy: TransferPhasePolicy,
    ) -> Result<()> {
        self.validate_shape()?;
        self.check_invariants()?;
        match phase_policy {
            TransferPhasePolicy::ActiveOnly => self.require_active()?,
            TransferPhasePolicy::ActiveOrResolved => {}
        }
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let i = Position::validate_outcome(self.outcomes, outcome)?;
        if from.internal[i] < quantity {
            return Err(Error::InsufficientBalance);
        }
        let new_from = from.internal[i]
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        let new_to = to.internal[i]
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        // The two deltas must be equal and opposite.  The sum is taken in u128
        // so the check stays exact even when the two balances jointly exceed
        // the `Amount` range.
        let before = u128::from(from.internal[i])
            .checked_add(u128::from(to.internal[i]))
            .ok_or(Error::ArithmeticOverflow)?;
        let after = u128::from(new_from)
            .checked_add(u128::from(new_to))
            .ok_or(Error::ArithmeticOverflow)?;
        if before != after {
            return Err(Error::InvariantViolation);
        }
        from.internal[i] = new_from;
        to.internal[i] = new_to;
        Ok(())
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
    use super::TransferPhasePolicy::{ActiveOnly, ActiveOrResolved};
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

    /// The P1-A fixture of `POLICY_ANALYSIS_LOTS_FEES.md` §1.1: one payout
    /// vector with weights `[1, 1]` over denominator 2, under which every
    /// single-outcome redemption of an odd quantity remainders forever.
    fn fractional_set() -> PayoutSet {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = 1;
        weights[1] = 1;
        vectors[0] = PayoutVector::new(2, weights);
        PayoutSet::new(1, 2, vectors)
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
    fn transfer_internal_conserves_supply_and_refuses_insufficient() {
        let mut market = MarketState::new(2, binary_set(), 0).unwrap();
        let mut seller = Position::EMPTY;
        let mut buyer = Position::EMPTY;
        market.split(&mut seller, 10).unwrap();
        let market_before = market;

        market
            .transfer_internal(&mut seller, &mut buyer, 1, 4, ActiveOnly)
            .unwrap();
        assert_eq!(seller.internal[1], 6);
        assert_eq!(buyer.internal[1], 4);
        assert_eq!(seller.internal[0], 10);
        // Supply and collateral neutrality is structural: `&self` cannot write.
        assert_eq!(market, market_before);
        assert_eq!(market.total_supply[1], 10);
        assert_eq!(market.collateral, 10);

        let seller_before = seller;
        let buyer_before = buyer;
        assert_eq!(
            market.transfer_internal(&mut seller, &mut buyer, 1, 0, ActiveOnly),
            Err(Error::ZeroQuantity)
        );
        assert_eq!(
            market.transfer_internal(&mut seller, &mut buyer, 2, 1, ActiveOnly),
            Err(Error::InvalidPayoutIndex)
        );
        assert_eq!(
            market.transfer_internal(&mut seller, &mut buyer, 1, 7, ActiveOnly),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(seller, seller_before);
        assert_eq!(buyer, buyer_before);
        assert_eq!(market, market_before);

        market
            .transfer_internal(&mut buyer, &mut seller, 1, 4, ActiveOnly)
            .unwrap();
        assert_eq!(seller.internal[1], 10);
        assert_eq!(buyer, Position::EMPTY);
        assert_eq!(market, market_before);
        market.check_invariants().unwrap();
    }

    #[test]
    fn transfer_internal_refuses_overflow_at_the_receiver() {
        let mut market = MarketState::new(2, binary_set(), 0).unwrap();
        let mut from = Position::EMPTY;
        let mut to = Position::EMPTY;
        market.split(&mut from, Amount::MAX).unwrap();
        // The receiver already holds the whole range; one more atom cannot land.
        to.internal[0] = Amount::MAX;
        assert_eq!(
            market.transfer_internal(&mut from, &mut to, 0, 1, ActiveOnly),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(from.internal[0], Amount::MAX);
        assert_eq!(to.internal[0], Amount::MAX);
    }

    #[test]
    fn transfer_internal_phase_policy_is_named_at_every_call() {
        let mut market = MarketState::new(2, binary_set(), 0).unwrap();
        let mut from = Position::EMPTY;
        let mut to = Position::EMPTY;
        market.split(&mut from, 6).unwrap();

        // T-a and T-b agree while the market is Active.
        market
            .transfer_internal(&mut from, &mut to, 0, 1, ActiveOnly)
            .unwrap();
        market
            .transfer_internal(&mut from, &mut to, 0, 1, ActiveOrResolved)
            .unwrap();
        assert_eq!(to.internal[0], 2);

        market.resolve(0).unwrap();
        let market_before = market;
        // T-a strands a settlement that races resolution ...
        assert_eq!(
            market.transfer_internal(&mut from, &mut to, 0, 2, ActiveOnly),
            Err(Error::AlreadyResolved)
        );
        assert_eq!(from.internal[0], 4);
        assert_eq!(to.internal[0], 2);
        // ... T-b settles it, and the receiver can still exit on its own.
        market
            .transfer_internal(&mut from, &mut to, 0, 2, ActiveOrResolved)
            .unwrap();
        assert_eq!(from.internal[0], 2);
        assert_eq!(to.internal[0], 4);
        assert_eq!(market, market_before);
        assert_eq!(market.redeem_internal(&mut to, 0, 4), Ok(4));
    }

    #[test]
    fn complete_set_redemption_exits_the_fractional_trap() {
        let mut market = MarketState::new(2, fractional_set(), 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 1).unwrap();
        market.resolve(0).unwrap();
        // P1-A: one atom of each outcome, and every single-outcome redemption
        // refuses forever while the collateral is solvent but exit-dead.
        assert_eq!(
            market.redeem_internal(&mut position, 0, 1),
            Err(Error::RemainderRequired)
        );
        assert_eq!(
            market.redeem_internal(&mut position, 1, 1),
            Err(Error::RemainderRequired)
        );
        assert_eq!(market.collateral, 1);
        // The complete set is exact, unconditionally.
        assert_eq!(market.redeem_complete_set(&mut position, 1), Ok(1));
        assert_eq!(market.collateral, 0);
        assert_eq!(market.total_supply, [0; MAX_OUTCOMES]);
        assert_eq!(position, Position::EMPTY);
        market.check_invariants().unwrap();
    }

    #[test]
    fn complete_set_redemption_conserves_collateral_and_supply() {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = 1;
        weights[1] = 3;
        vectors[0] = PayoutVector::new(4, weights);
        let mut market = MarketState::new(2, PayoutSet::new(1, 2, vectors), 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 9).unwrap();
        market.resolve(0).unwrap();

        // Hoard falls by exactly one per-set atom per set, every supply falls
        // by the redeemed quantity, and nothing remainders.
        assert_eq!(market.redeem_complete_set(&mut position, 5), Ok(5));
        assert_eq!(market.collateral, 4);
        assert_eq!(market.total_supply[0], 4);
        assert_eq!(market.total_supply[1], 4);
        assert_eq!(position.internal[0], 4);
        assert_eq!(position.internal[1], 4);
        assert_eq!(market.required_collateral().unwrap(), 4);

        assert_eq!(market.redeem_complete_set(&mut position, 4), Ok(4));
        assert_eq!(market.collateral, 0);
        assert_eq!(market.total_supply, [0; MAX_OUTCOMES]);
        assert_eq!(position, Position::EMPTY);
        market.check_invariants().unwrap();
    }

    #[test]
    fn complete_set_redemption_refuses_wrong_phase_and_partial_sets() {
        let mut market = MarketState::new(2, fractional_set(), 0).unwrap();
        let mut position = Position::EMPTY;
        let mut other = Position::EMPTY;
        market.split(&mut position, 3).unwrap();
        assert_eq!(
            market.redeem_complete_set(&mut position, 1),
            Err(Error::NotResolved)
        );
        market
            .transfer_internal(&mut position, &mut other, 0, 1, ActiveOnly)
            .unwrap();
        market.resolve(0).unwrap();
        assert_eq!(
            market.redeem_complete_set(&mut position, 0),
            Err(Error::ZeroQuantity)
        );
        // Three sets are not held: the short leg is refused as a balance fault.
        assert_eq!(
            market.redeem_complete_set(&mut position, 3),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(market.redeem_complete_set(&mut position, 2), Ok(2));
        assert_eq!(market.collateral, 1);
        assert_eq!(position.internal[1], 1);
        // An *unbalanced* fractional holding is still exit-dead: §1.5 rescues
        // balanced positions only, and the divisibility policy stays unfrozen.
        assert_eq!(
            market.redeem_internal(&mut other, 0, 1),
            Err(Error::RemainderRequired)
        );
        market.check_invariants().unwrap();
    }

    #[test]
    fn refused_active_transitions_leave_market_and_position_unchanged() {
        let mut market = MarketState::new(2, binary_set(), 50).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 12).unwrap();
        let market_before = market;
        let position_before = position;

        assert_eq!(market.split(&mut position, 0), Err(Error::ZeroQuantity));
        assert_eq!(
            market.split(&mut position, Amount::MAX),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(
            market.merge(&mut position, 13),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(
            market.materialize(&mut position, 0, 13),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(
            market.materialize(&mut position, 4, 1),
            Err(Error::InvalidPayoutIndex)
        );
        assert_eq!(
            market.dematerialize(&mut position, 0, 1),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(market.resolve(9), Err(Error::InvalidPayoutIndex));
        assert_eq!(
            market.redeem_internal(&mut position, 0, 1),
            Err(Error::NotResolved)
        );
        assert_eq!(
            market.redeem_external(&mut position, 0, 1),
            Err(Error::NotResolved)
        );
        assert_eq!(
            market.redeem_complete_set(&mut position, 1),
            Err(Error::NotResolved)
        );
        assert_eq!(market, market_before);
        assert_eq!(position, position_before);
    }

    #[test]
    fn refused_resolved_transitions_leave_market_and_position_unchanged() {
        let mut market = MarketState::new(2, fractional_set(), 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 3).unwrap();
        market.materialize(&mut position, 1, 1).unwrap();
        market.resolve(0).unwrap();
        let market_before = market;
        let position_before = position;

        assert_eq!(
            market.redeem_internal(&mut position, 0, 1),
            Err(Error::RemainderRequired)
        );
        assert_eq!(
            market.redeem_external(&mut position, 1, 2),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(
            market.redeem_complete_set(&mut position, 4),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(
            market.redeem_complete_set(&mut position, 0),
            Err(Error::ZeroQuantity)
        );
        assert_eq!(market.split(&mut position, 1), Err(Error::AlreadyResolved));
        assert_eq!(market.merge(&mut position, 1), Err(Error::AlreadyResolved));
        assert_eq!(
            market.materialize(&mut position, 0, 1),
            Err(Error::AlreadyResolved)
        );
        assert_eq!(
            market.dematerialize(&mut position, 1, 1),
            Err(Error::AlreadyResolved)
        );
        assert_eq!(market.resolve(0), Err(Error::AlreadyResolved));
        assert_eq!(market, market_before);
        assert_eq!(position, position_before);
    }

    #[test]
    fn merge_reports_the_collateral_fault_before_the_balance_fault() {
        // R8 is landed behavior, pinned here so a reorder cannot pass silently.
        let mut market = MarketState::new(2, binary_set(), 0).unwrap();
        let mut position = Position::EMPTY;
        assert_eq!(
            market.merge(&mut position, 1),
            Err(Error::InsufficientCollateral)
        );

        // With surplus collateral the balance test is the one that speaks.
        let mut funded = MarketState::new(2, binary_set(), 10).unwrap();
        funded.split(&mut position, 5).unwrap();
        assert_eq!(
            funded.merge(&mut position, 6),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(funded.collateral, 15);
        assert_eq!(position.internal[0], 5);
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
