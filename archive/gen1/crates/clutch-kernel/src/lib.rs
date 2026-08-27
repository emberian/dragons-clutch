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
//! The kernel intentionally has no discretionary resolver.  A market freezes
//! one resolution seam at construction ([`BasisMode`]) and has exactly that
//! one: a `FinitePreset` market resolves only by index into the immutable
//! finite payout set, and a `DerivedBasis` market resolves only by a weight
//! vector that validates against the frozen common denominator.  Neither seam
//! claims that a source, adapter, or external observation is authentic, and
//! neither reads more of a payout vector than nonnegativity, the per-weight
//! bound, and exact sum-to-`D` — hypotheses (H1) and (H2) of
//! `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md` §3.
//!
//! `Amount` is an opaque collateral-atom quantity.  No collateral mint,
//! decimal system, or asset-specific rule is embedded in this crate.

use core::convert::TryFrom;

mod transfer_arithmetic;

use transfer_arithmetic::{prepare_internal_transfer, TransferArithmeticError};

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
    /// The resolution seam does not belong to this market's frozen
    /// [`BasisMode`]: `resolve` on a `DerivedBasis` market, or
    /// `resolve_with_vector` on a `FinitePreset` one.
    ///
    /// Appended rather than inserted: an adapter that maps this enum by
    /// discriminant (the SBF program's `0x2000 + n` block) keeps every
    /// previously assigned number.
    WrongResolutionMode,
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

/// The single resolution seam a market freezes at construction.
///
/// `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md` §4 piece 1.  There is
/// deliberately no `Default`: which seam a market has is a terms decision the
/// caller names at [`MarketState::new`], never one the kernel picks, and a
/// market that got its mode from a default would be a market whose resolution
/// story nobody wrote down.  Frozen at construction and never written again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BasisMode {
    /// Mode 0: resolution names an index into the immutable finite payout set.
    /// Byte-for-byte the semantics this kernel had before mode 1 existed.
    FinitePreset = 0,
    /// Mode 1: resolution installs a derived weight vector that the kernel
    /// validates for shape but does not bind to evidence — that binding is the
    /// adapter's derivation, exactly as binding an index to evidence is in
    /// mode 0.  The kernel-alone claim narrows from "payout is one of the
    /// frozen 8" to "payout is a member of the frozen simplex lattice
    /// `{w : 0 <= w_i <= D, sum w_i = D}`" (§3.3).
    DerivedBasis = 1,
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
    /// Index in `payouts` after resolution; meaningless while Active, and
    /// always zero in [`BasisMode::DerivedBasis`], which names no index.
    pub resolved_payout: u8,
    /// The frozen resolution seam.  Written once, by [`MarketState::new`].
    pub basis_mode: BasisMode,
    /// The installed payout vector in [`BasisMode::DerivedBasis`] after
    /// resolution.  [`PayoutVector::ZERO`] while Active, and
    /// [`PayoutVector::ZERO`] in [`BasisMode::FinitePreset`] in either phase:
    /// mode 0 stores no second copy of any weight.
    pub resolved_vector: PayoutVector,
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
    /// Construct an Active market with a frozen payout set and a frozen
    /// resolution seam.
    ///
    /// `basis_mode` is a required argument for the reason `TransferPhasePolicy`
    /// is one at [`MarketState::transfer_internal`]: the kernel holds no
    /// default opinion about a policy the terms own.  The mode is validated
    /// here, with every other field, by the constructor's own
    /// `check_invariants`.
    pub fn new(
        outcomes: u8,
        basis_mode: BasisMode,
        payouts: PayoutSet,
        collateral: Amount,
    ) -> Result<Self> {
        if outcomes != payouts.outcomes {
            return Err(Error::InvalidOutcomeCount);
        }
        payouts.validate()?;
        let state = Self {
            outcomes,
            phase: Phase::Active,
            resolved_payout: 0,
            basis_mode,
            resolved_vector: PayoutVector::ZERO,
            collateral,
            total_supply: [0; MAX_OUTCOMES],
            payouts,
        };
        state.check_invariants()?;
        Ok(state)
    }

    pub fn required_collateral(&self) -> Result<Amount> {
        self.required_collateral_for(
            &self.total_supply,
            self.phase,
            self.resolved_payout,
            &self.resolved_vector,
        )
    }

    /// `required_collateral` evaluated against a prospective supply and
    /// resolution without materializing that state.  Transitions use it to
    /// judge their own result before they write it.
    fn required_collateral_for(
        &self,
        total_supply: &[Amount; MAX_OUTCOMES],
        phase: Phase,
        resolved_payout: u8,
        resolved_vector: &PayoutVector,
    ) -> Result<Amount> {
        self.validate_shape()?;
        match (phase, self.basis_mode) {
            (Phase::Active, BasisMode::FinitePreset) => {
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
            (Phase::Active, BasisMode::DerivedBasis) => {
                // The mode-1 Active requirement of the design's §4 piece 3:
                // `max_i T_i`.  By Theorem (iv) this is the exact supremum of
                // `required_resolved` over the whole frozen simplex lattice,
                // not an over-reservation chosen for safety, and it dominates
                // every preset's liability too, so mode 1's Active requirement
                // is never weaker than mode 0's over the same presets.
                let mut max = 0_u64;
                let mut i = 0_usize;
                while i < usize::from(self.outcomes) {
                    if total_supply[i] > max {
                        max = total_supply[i];
                    }
                    i += 1;
                }
                Ok(max)
            }
            (Phase::Resolved, BasisMode::FinitePreset) => {
                self.required_for_vector(total_supply, self.payouts.get(resolved_payout)?)
            }
            (Phase::Resolved, BasisMode::DerivedBasis) => {
                self.required_for_vector(total_supply, resolved_vector)
            }
        }
    }

    /// The payout vector a resolved market actually pays from.
    ///
    /// The design's §4 piece 3 accessor: the preset named by index in mode 0,
    /// the installed vector in mode 1.  `redeem`, `redeem_complete_set`, and
    /// the Resolved arm of `required_collateral_for` read the resolved payout
    /// only through here, so neither redemption path knows which mode it is
    /// serving.
    fn effective_resolved_vector(&self) -> Result<&PayoutVector> {
        match self.basis_mode {
            BasisMode::FinitePreset => self.payouts.get(self.resolved_payout),
            BasisMode::DerivedBasis => {
                if self.phase != Phase::Resolved {
                    return Err(Error::NotResolved);
                }
                Ok(&self.resolved_vector)
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
        if denominator == 0 {
            // Defense in depth: every vector that reaches here has passed
            // `PayoutVector::validate`, which refuses a zero denominator.  The
            // guard keeps the function total rather than trusting that.
            return Err(Error::InvalidDenominator);
        }
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
            &self.resolved_vector,
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
        resolved_vector: &PayoutVector,
    ) -> Result<()> {
        self.validate_shape()
            .map_err(|_| Error::InvariantViolation)?;
        self.validate_resolution(phase, resolved_payout, resolved_vector)
            .map_err(|_| Error::InvariantViolation)?;
        // The pre-mode rule, verbatim and in its original place: a resolved
        // mode-0 index must be inside the frozen set, reported as an invariant
        // violation rather than as a shape fault.
        if self.basis_mode == BasisMode::FinitePreset
            && phase == Phase::Resolved
            && resolved_payout >= self.payouts.count
        {
            return Err(Error::InvariantViolation);
        }
        let required =
            self.required_collateral_for(total_supply, phase, resolved_payout, resolved_vector)?;
        if collateral < required {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<()> {
        if self.outcomes != self.payouts.outcomes {
            return Err(Error::InvalidOutcomeCount);
        }
        self.payouts.validate()?;
        self.validate_resolution(self.phase, self.resolved_payout, &self.resolved_vector)
    }

    /// The structural rules the two resolution *slots* obey, in either phase.
    ///
    /// Taken over prospective values so a transition can judge the resolution
    /// it is about to write before it writes it.  The mode owns which slot may
    /// be non-empty: mode 0 never carries a vector, and mode 1 never carries an
    /// index.  This is what makes "one resolution seam per mode, never both"
    /// a property of every reachable state rather than of the two entry points
    /// alone — a caller that writes the public fields directly is refused here.
    fn validate_resolution(
        &self,
        phase: Phase,
        resolved_payout: u8,
        resolved_vector: &PayoutVector,
    ) -> Result<()> {
        match self.basis_mode {
            BasisMode::FinitePreset => {
                // The only rule mode 0 gained: it stores no vector, in either
                // phase.  Everything else about a mode-0 state — including
                // where the resolved index is bounds-checked and with which
                // refusal — is exactly what it was before mode 1 existed.
                if *resolved_vector != PayoutVector::ZERO {
                    return Err(Error::InvalidPayoutWeights);
                }
            }
            BasisMode::DerivedBasis => {
                if resolved_payout != 0 {
                    return Err(Error::InvalidPayoutIndex);
                }
                match phase {
                    Phase::Active => {
                        if *resolved_vector != PayoutVector::ZERO {
                            return Err(Error::InvalidPayoutWeights);
                        }
                    }
                    Phase::Resolved => {
                        // (H1) and (H2) against the market's frozen `D`, which
                        // `PayoutSet::validate` has already proved common to
                        // every preset.
                        if resolved_vector.denominator != self.payouts.vectors[0].denominator {
                            return Err(Error::InvalidDenominator);
                        }
                        resolved_vector.validate(self.outcomes)?;
                    }
                }
            }
        }
        Ok(())
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
            &self.resolved_vector,
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
            &self.resolved_vector,
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
    /// The [`BasisMode::FinitePreset`] seam.  A [`BasisMode::DerivedBasis`]
    /// market refuses `Error::WrongResolutionMode` here and resolves through
    /// [`MarketState::resolve_with_vector`] instead: one resolution seam per
    /// mode, never both.  The gate sits after the phase gate, so an
    /// already-resolved market still reports `AlreadyResolved` first.
    ///
    /// Every check completes before the first write, the prospective invariant
    /// check included: on `Err`, `self` is unchanged.
    pub fn resolve(&mut self, payout_index: u8) -> Result<()> {
        self.validate_shape()?;
        self.check_invariants()?;
        self.require_active()?;
        if self.basis_mode != BasisMode::FinitePreset {
            return Err(Error::WrongResolutionMode);
        }
        self.payouts.get(payout_index)?;
        self.check_invariants_for(
            self.collateral,
            &self.total_supply,
            Phase::Resolved,
            payout_index,
            &PayoutVector::ZERO,
        )?;
        self.phase = Phase::Resolved;
        self.resolved_payout = payout_index;
        Ok(())
    }

    /// Fix the resolved payout to a derived, validated vector
    /// ([`BasisMode::DerivedBasis`] only).
    ///
    /// The kernel checks shape, not provenance: the vector must carry the
    /// market's frozen common denominator `D` and validate over the active
    /// outcomes — weights nonnegative and at most `D`, zero beyond the active
    /// prefix, summing to exactly `D`, which are hypotheses (H1) and (H2) of
    /// `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md` §3.1.  Binding the
    /// vector to evidence is the adapter's derivation, exactly as binding an
    /// index to evidence is in mode 0.  The kernel does not know what a knot,
    /// a degree, or a resolved value is, and gains no way to learn.
    ///
    /// The prospective invariant check is defense in depth rather than a live
    /// refusal: by Theorem (i) of §3.2,
    /// `required_resolved(T, w) <= required_active(T) = max_i T_i` for every
    /// admitted vector, so an Active market that held its invariant cannot
    /// breach it by resolving.  It is checked anyway, and the falsifier
    /// `mode_one_resolution_never_raises_the_requirement` is the exhaustive
    /// form of the theorem over small lattices.
    ///
    /// Every check completes before the first write, the prospective invariant
    /// check included: on `Err`, `self` is unchanged — including
    /// `resolved_vector`, which is written last.
    pub fn resolve_with_vector(&mut self, vector: PayoutVector) -> Result<()> {
        self.validate_shape()?;
        self.check_invariants()?;
        self.require_active()?;
        if self.basis_mode != BasisMode::DerivedBasis {
            return Err(Error::WrongResolutionMode);
        }
        // The frozen `D`.  `PayoutSet::validate` has already proved `count >= 1`
        // and that every member carries this same denominator, so vector zero
        // is the market's denominator and not merely one opinion of it.
        if vector.denominator != self.payouts.vectors[0].denominator {
            return Err(Error::InvalidDenominator);
        }
        vector.validate(self.outcomes)?;
        self.check_invariants_for(
            self.collateral,
            &self.total_supply,
            Phase::Resolved,
            0,
            &vector,
        )?;
        self.phase = Phase::Resolved;
        self.resolved_payout = 0;
        self.resolved_vector = vector;
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
        let vector = *self.effective_resolved_vector()?;
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
            &self.resolved_vector,
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
        let vector = *self.effective_resolved_vector()?;
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
            &self.resolved_vector,
        )?;
        self.collateral = new_collateral;
        self.total_supply = next_supply;
        position.internal = next_internal;
        Ok(payout)
    }

    /// Donate collateral atoms to the Hoard without minting claims.
    ///
    /// This transition cannot create an entitlement, fee, bounty, reserve, or
    /// caller credit. It exists so another custody kernel can return abandoned
    /// backing without becoming a second owner of the base collateral
    /// invariant. Every check completes before the first write.
    pub fn donate_collateral(&mut self, quantity: Amount) -> Result<()> {
        self.validate_shape()?;
        self.check_invariants()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let next_collateral = self
            .collateral
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        self.check_invariants_for(
            next_collateral,
            &self.total_supply,
            self.phase,
            self.resolved_payout,
            &self.resolved_vector,
        )?;
        self.collateral = next_collateral;
        Ok(())
    }

    /// Destroy an exact nonzero vector of internal claims without releasing
    /// collateral.
    ///
    /// This is the claim side of a beneficiary-free donation. Active or
    /// Resolved claims may be destroyed because reducing liabilities while
    /// retaining collateral cannot weaken the base invariant. Quantities must
    /// be canonically padded. On `Err`, market and position are unchanged.
    pub fn donate_internal_vector(
        &mut self,
        position: &mut Position,
        quantities: [Amount; MAX_OUTCOMES],
    ) -> Result<()> {
        self.validate_shape()?;
        self.check_invariants()?;
        let count = usize::from(self.outcomes);
        let mut any = false;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let quantity = quantities[index];
            if index < count {
                any |= quantity != 0;
                if position.internal[index] < quantity || self.total_supply[index] < quantity {
                    return Err(Error::InsufficientBalance);
                }
            } else if quantity != 0 {
                return Err(Error::InvalidPayoutWeights);
            }
            index += 1;
        }
        if !any {
            return Err(Error::ZeroQuantity);
        }
        let mut next_supply = self.total_supply;
        let mut next_internal = position.internal;
        index = 0;
        while index < count {
            next_supply[index] = next_supply[index]
                .checked_sub(quantities[index])
                .ok_or(Error::ArithmeticUnderflow)?;
            next_internal[index] = next_internal[index]
                .checked_sub(quantities[index])
                .ok_or(Error::ArithmeticUnderflow)?;
            index += 1;
        }
        self.check_invariants_for(
            self.collateral,
            &next_supply,
            self.phase,
            self.resolved_payout,
            &self.resolved_vector,
        )?;
        self.total_supply = next_supply;
        position.internal = next_internal;
        Ok(())
    }

    /// Redeem an aggregate internal vector at its exact terminal value.
    ///
    /// Individual legs may have fractional payouts; only the vector's summed
    /// numerator must divide the resolved denominator. This is the general
    /// atomic exit needed by nonnegative structured portfolios. It names the
    /// kernel's existing exactness boundary (`RemainderRequired`) and never
    /// floors. On `Err`, market and position are unchanged.
    pub fn redeem_internal_vector_exact(
        &mut self,
        position: &mut Position,
        quantities: [Amount; MAX_OUTCOMES],
    ) -> Result<Amount> {
        self.validate_shape()?;
        if self.phase != Phase::Resolved {
            return Err(Error::NotResolved);
        }
        self.check_invariants()?;
        let count = usize::from(self.outcomes);
        let vector = *self.effective_resolved_vector()?;
        let mut numerator = 0_u128;
        let mut any = false;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let quantity = quantities[index];
            if index < count {
                any |= quantity != 0;
                if position.internal[index] < quantity || self.total_supply[index] < quantity {
                    return Err(Error::InsufficientBalance);
                }
                let term = u128::from(quantity)
                    .checked_mul(u128::from(vector.weights[index]))
                    .ok_or(Error::ArithmeticOverflow)?;
                numerator = numerator
                    .checked_add(term)
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if quantity != 0 {
                return Err(Error::InvalidPayoutWeights);
            }
            index += 1;
        }
        if !any {
            return Err(Error::ZeroQuantity);
        }
        let denominator = u128::from(vector.denominator);
        if !numerator.is_multiple_of(denominator) {
            return Err(Error::RemainderRequired);
        }
        let payout =
            Amount::try_from(numerator / denominator).map_err(|_| Error::ArithmeticOverflow)?;
        if self.collateral < payout {
            return Err(Error::InsufficientCollateral);
        }
        let mut next_supply = self.total_supply;
        let mut next_internal = position.internal;
        index = 0;
        while index < count {
            next_supply[index] = next_supply[index]
                .checked_sub(quantities[index])
                .ok_or(Error::ArithmeticUnderflow)?;
            next_internal[index] = next_internal[index]
                .checked_sub(quantities[index])
                .ok_or(Error::ArithmeticUnderflow)?;
            index += 1;
        }
        let next_collateral = self
            .collateral
            .checked_sub(payout)
            .ok_or(Error::ArithmeticUnderflow)?;
        self.check_invariants_for(
            next_collateral,
            &next_supply,
            self.phase,
            self.resolved_payout,
            &self.resolved_vector,
        )?;
        self.total_supply = next_supply;
        self.collateral = next_collateral;
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
        // VERUS-TRANSFER-CALLSITE-BEGIN: digest-bound by run_transfer_refinement.sh.
        if from.internal[i] < quantity {
            return Err(Error::InsufficientBalance);
        }
        let (new_from, new_to) =
            prepare_internal_transfer(from.internal[i], to.internal[i], quantity).map_err(
                |error| match error {
                    TransferArithmeticError::Overflow => Error::ArithmeticOverflow,
                    TransferArithmeticError::Underflow => Error::ArithmeticUnderflow,
                    TransferArithmeticError::Conservation => Error::InvariantViolation,
                },
            )?;
        from.internal[i] = new_from;
        to.internal[i] = new_to;
        // VERUS-TRANSFER-CALLSITE-END
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
    use super::BasisMode::{DerivedBasis, FinitePreset};
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
        let mut market = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, set, 0).unwrap();
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
            MarketState::new(2, FinitePreset, PayoutSet::new(1, 2, vectors), 0),
            Err(Error::InvalidPayoutWeights)
        );

        let mut market = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, fractional_set(), 0).unwrap();
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
        let mut market =
            MarketState::new(2, FinitePreset, PayoutSet::new(1, 2, vectors), 0).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, fractional_set(), 0).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, binary_set(), 50).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, fractional_set(), 0).unwrap();
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
        let mut market = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
        let mut position = Position::EMPTY;
        assert_eq!(
            market.merge(&mut position, 1),
            Err(Error::InsufficientCollateral)
        );

        // With surplus collateral the balance test is the one that speaks.
        let mut funded = MarketState::new(2, FinitePreset, binary_set(), 10).unwrap();
        funded.split(&mut position, 5).unwrap();
        assert_eq!(
            funded.merge(&mut position, 6),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(funded.collateral, 15);
        assert_eq!(position.internal[0], 5);
    }

    /// The minimal `DerivedBasis` payout set: one *named* vector, which is all
    /// the design's §4 asks of mode 1's `PayoutSet` — it anchors the frozen
    /// common denominator `D` and stands in for the frozen failure-refund
    /// vector.  The reachable lattice is not enumerated here and cannot be.
    fn anchor_set(denominator: u64, outcomes: u8) -> PayoutSet {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = denominator;
        vectors[0] = PayoutVector::new(denominator, weights);
        PayoutSet::new(1, outcomes, vectors)
    }

    fn weights_of(pairs: &[u64]) -> [u64; MAX_OUTCOMES] {
        let mut weights = [0_u64; MAX_OUTCOMES];
        let mut i = 0_usize;
        while i < pairs.len() {
            weights[i] = pairs[i];
            i += 1;
        }
        weights
    }

    /// Theorem (i) of `DISTRIBUTIONAL_CLAIMS_DESIGN.md` §3.2, as a bounded
    /// exhaustive falsifier rather than as a proof sketch:
    /// `required_resolved(T, w) <= required_active(T)` for every supply shape
    /// and every admitted vector of the frozen lattice.
    ///
    /// It is run through the kernel's own transition, not through a
    /// re-implementation of the arithmetic: each market is funded to *exactly*
    /// its Active requirement — the tight, adversarial case — so any vector
    /// whose resolved liability exceeded it would surface as the
    /// `InvariantViolation` the prospective check raises.  A pass is therefore
    /// the claim that the prospective check inside `resolve_with_vector` is
    /// unreachable over these lattices, which is what "defense in depth, not a
    /// live refusal" means.
    #[test]
    fn mode_one_resolution_never_raises_the_requirement() {
        let mut checked = 0_u64;
        for denominator in [2_u64, 4, 8, 16] {
            let set = anchor_set(denominator, 2);
            for right in 0..=denominator {
                let vector =
                    PayoutVector::new(denominator, weights_of(&[denominator - right, right]));
                for first in 0..=20_u64 {
                    for second in 0..=20_u64 {
                        let required_active = if first > second { first } else { second };
                        let mut market =
                            MarketState::new(2, DerivedBasis, set, required_active).unwrap();
                        market.total_supply[0] = first;
                        market.total_supply[1] = second;
                        // (DEF) required_active(T) = max_i T_i, exactly.
                        assert_eq!(market.required_collateral(), Ok(required_active));
                        market.resolve_with_vector(vector).unwrap();
                        let resolved = market.required_collateral().unwrap();
                        assert!(
                            resolved <= required_active,
                            "D={denominator} w=({},{right}) T=({first},{second}): {resolved} > {required_active}",
                            denominator - right
                        );
                        // The ceiling is the one named rounding boundary.
                        let exact = first * (denominator - right) + second * right;
                        assert_eq!(resolved, exact.div_ceil(denominator));
                        checked += 1;
                    }
                }
            }
        }

        // The same claim over three-outcome lattices, where a vector can put
        // weight on an outcome that carries neither supply extreme.
        for denominator in [2_u64, 4, 8] {
            let set = anchor_set(denominator, 3);
            for first_weight in 0..=denominator {
                for second_weight in 0..=(denominator - first_weight) {
                    let third_weight = denominator - first_weight - second_weight;
                    let vector = PayoutVector::new(
                        denominator,
                        weights_of(&[first_weight, second_weight, third_weight]),
                    );
                    for a in 0..=8_u64 {
                        for b in 0..=8_u64 {
                            for c in 0..=8_u64 {
                                let required_active = a.max(b).max(c);
                                let mut market =
                                    MarketState::new(3, DerivedBasis, set, required_active)
                                        .unwrap();
                                market.total_supply[0] = a;
                                market.total_supply[1] = b;
                                market.total_supply[2] = c;
                                assert_eq!(market.required_collateral(), Ok(required_active));
                                market.resolve_with_vector(vector).unwrap();
                                assert!(market.required_collateral().unwrap() <= required_active);
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        // 14,994 two-outcome cases (34 vectors over four denominators, 441
        // supply shapes each) plus 48,114 three-outcome cases (66 vectors over
        // three denominators, 729 supply shapes each).  Pinned exactly so a
        // narrowed loop bound fails loudly instead of silently sampling.
        assert_eq!(checked, 63_108);
    }

    /// Mode 1's Active requirement is `max_i T_i`, and that is never weaker
    /// than the mode-0 requirement over the same presets (§4 piece 3).
    #[test]
    fn derived_basis_active_requirement_is_the_supply_maximum() {
        let mut derived = MarketState::new(2, DerivedBasis, anchor_set(7, 2), 0).unwrap();
        let mut position = Position::EMPTY;
        derived.split(&mut position, 9).unwrap();
        assert_eq!(derived.required_collateral(), Ok(9));
        // Split moves every outcome together, so equal supplies make the
        // requirement equal the collateral exactly, as in mode 0.
        assert_eq!(derived.collateral, 9);
        // An unequal aggregate is the case that separates the two arms: the
        // preset maximum reads one weight vector, `max_i T_i` reads the shape.
        derived.total_supply[1] = 3;
        assert_eq!(derived.required_collateral(), Ok(9));

        let preset = MarketState::new(2, FinitePreset, anchor_set(7, 2), 0).unwrap();
        let mut same = preset;
        same.total_supply[0] = 9;
        same.total_supply[1] = 3;
        // The one frozen preset pays outcome 0 in full, so mode 0 requires 9
        // here too — and can never require more, because `max_i T_i` bounds
        // every preset's liability by Theorem (i).
        assert_eq!(same.required_collateral(), Ok(9));
        assert!(same.required_collateral().unwrap() <= derived.required_collateral().unwrap());
    }

    /// Derive-and-subtract exactness carried through resolution and
    /// redemption: the `(5, 2)` over `D = 7` vector is the design §15
    /// deg-1 derivation at `x̂ = 3` on the `[0, 8)` pane, and it is a vector no
    /// eight-member preset set has to enumerate.
    #[test]
    fn derived_resolution_pays_the_derived_fractions_exactly() {
        let vector = PayoutVector::new(7, weights_of(&[5, 2]));
        let mut market = MarketState::new(2, DerivedBasis, anchor_set(7, 2), 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 14).unwrap();
        market.resolve_with_vector(vector).unwrap();
        assert_eq!(market.phase, Phase::Resolved);
        assert_eq!(market.resolved_vector, vector);
        // Mode 1 names no index; the slot stays canonical.
        assert_eq!(market.resolved_payout, 0);

        // 14 * 5 / 7 = 10 and 14 * 2 / 7 = 4, both exact, together the whole
        // split collateral.
        assert_eq!(market.redeem_internal(&mut position, 0, 14), Ok(10));
        assert_eq!(market.collateral, 4);
        assert_eq!(market.redeem_internal(&mut position, 1, 14), Ok(4));
        assert_eq!(market.collateral, 0);
        assert_eq!(position, Position::EMPTY);
        market.check_invariants().unwrap();
    }

    /// The remainder refusal and the complete-set exit are untouched by mode 1:
    /// a single-outcome redemption that is not an exact number of atoms still
    /// refuses rather than flooring, and a balanced holder still exits exactly
    /// (Theorem (ii): `sum_i q * w_i = q * D` at every resolved vector).
    #[test]
    fn derived_resolution_keeps_remainder_refusal_and_complete_set_exit() {
        let vector = PayoutVector::new(7, weights_of(&[5, 2]));
        let mut market = MarketState::new(2, DerivedBasis, anchor_set(7, 2), 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 3).unwrap();
        market.resolve_with_vector(vector).unwrap();
        // 3 * 5 / 7 and 3 * 2 / 7 both remainder; neither leg is redeemable
        // alone, for any quantity the holder has.
        assert_eq!(
            market.redeem_internal(&mut position, 0, 3),
            Err(Error::RemainderRequired)
        );
        assert_eq!(
            market.redeem_internal(&mut position, 1, 1),
            Err(Error::RemainderRequired)
        );
        // The complete set never remainders.
        assert_eq!(market.redeem_complete_set(&mut position, 3), Ok(3));
        assert_eq!(market.collateral, 0);
        assert_eq!(position, Position::EMPTY);
        market.check_invariants().unwrap();
    }

    /// One resolution seam per mode, never both, in both directions.
    #[test]
    fn resolution_seams_refuse_across_modes() {
        let vector = PayoutVector::new(1, weights_of(&[1, 0]));
        let mut categorical = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
        assert_eq!(
            categorical.resolve_with_vector(vector),
            Err(Error::WrongResolutionMode)
        );
        categorical.resolve(1).unwrap();

        let mut derived = MarketState::new(2, DerivedBasis, anchor_set(7, 2), 0).unwrap();
        assert_eq!(derived.resolve(0), Err(Error::WrongResolutionMode));
        derived
            .resolve_with_vector(PayoutVector::new(7, weights_of(&[4, 3])))
            .unwrap();

        // The phase gate still speaks first on both seams: an already-resolved
        // market reports `AlreadyResolved`, not the mode.
        assert_eq!(
            categorical.resolve_with_vector(vector),
            Err(Error::AlreadyResolved)
        );
        assert_eq!(derived.resolve(0), Err(Error::AlreadyResolved));
    }

    /// The vector gate is the existing `PayoutVector` shape rules against the
    /// frozen `D`, and every refusal leaves the market byte-identical.
    #[test]
    fn refused_resolve_with_vector_leaves_the_market_unchanged() {
        let mut market = MarketState::new(2, DerivedBasis, anchor_set(7, 2), 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 14).unwrap();
        let before = market;
        let position_before = position;

        // A different denominator is not this market's `D`, even though the
        // vector is a perfectly good simplex point on its own.
        assert_eq!(
            market.resolve_with_vector(PayoutVector::new(8, weights_of(&[5, 3]))),
            Err(Error::InvalidDenominator)
        );
        // Sum is not `D` (H2 fails).
        assert_eq!(
            market.resolve_with_vector(PayoutVector::new(7, weights_of(&[5, 3]))),
            Err(Error::InvalidPayoutWeights)
        );
        // A single weight exceeds `D` (H1 fails).
        assert_eq!(
            market.resolve_with_vector(PayoutVector::new(7, weights_of(&[8, 0]))),
            Err(Error::InvalidPayoutWeights)
        );
        // Nonzero weight beyond the active prefix.
        assert_eq!(
            market.resolve_with_vector(PayoutVector::new(7, weights_of(&[5, 1, 1]))),
            Err(Error::InvalidPayoutWeights)
        );
        // The zero vector carries no denominator at all.
        assert_eq!(
            market.resolve_with_vector(PayoutVector::ZERO),
            Err(Error::InvalidDenominator)
        );
        // Full-struct equality: not one field of the market moved, the
        // `resolved_vector` slot included, and the position is untouched.
        assert_eq!(market, before);
        assert_eq!(position, position_before);
        assert_eq!(market.resolved_vector, PayoutVector::ZERO);
        assert_eq!(market.phase, Phase::Active);

        // An under-collateralized market refuses at the invariant, also
        // without writing.
        let mut thin = MarketState::new(2, DerivedBasis, anchor_set(7, 2), 0).unwrap();
        thin.total_supply[0] = 5;
        let thin_before = thin;
        assert_eq!(
            thin.resolve_with_vector(PayoutVector::new(7, weights_of(&[7, 0]))),
            Err(Error::InvariantViolation)
        );
        assert_eq!(thin, thin_before);
    }

    /// The mode owns which resolution slot may be non-empty, in every state and
    /// not only at the two entry points: a forged state that carries both, or
    /// the wrong one, is refused by every public operation.
    #[test]
    fn forged_resolution_slots_are_refused_in_both_modes() {
        let mut position = Position::EMPTY;

        let mut preset = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
        preset.resolved_vector = PayoutVector::new(1, weights_of(&[1, 0]));
        assert_eq!(
            preset.required_collateral(),
            Err(Error::InvalidPayoutWeights)
        );
        assert_eq!(
            preset.split(&mut position, 1),
            Err(Error::InvalidPayoutWeights)
        );
        assert_eq!(preset.resolve(0), Err(Error::InvalidPayoutWeights));

        let mut derived = MarketState::new(2, DerivedBasis, anchor_set(7, 2), 0).unwrap();
        derived.resolved_payout = 3;
        assert_eq!(
            derived.required_collateral(),
            Err(Error::InvalidPayoutIndex)
        );
        assert_eq!(
            derived.resolve_with_vector(PayoutVector::new(7, weights_of(&[7, 0]))),
            Err(Error::InvalidPayoutIndex)
        );

        // A resolved mode-1 market whose installed vector is later corrupted
        // fails the same (H1)/(H2) gate the transition applied.
        let mut resolved = MarketState::new(2, DerivedBasis, anchor_set(7, 2), 0).unwrap();
        resolved.split(&mut position, 7).unwrap();
        resolved
            .resolve_with_vector(PayoutVector::new(7, weights_of(&[3, 4])))
            .unwrap();
        resolved.resolved_vector = PayoutVector::new(7, weights_of(&[3, 3]));
        assert_eq!(
            resolved.redeem_internal(&mut position, 0, 7),
            Err(Error::InvalidPayoutWeights)
        );
    }

    #[test]
    fn repeated_small_traces_preserve_invariant() {
        let mut market = MarketState::new(
            3,
            FinitePreset,
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
                FinitePreset,
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

    #[test]
    fn donation_transitions_are_beneficiary_free_phase_independent_and_atomic() {
        let mut market = MarketState::new(2, FinitePreset, binary_set(), 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 5).unwrap();
        market.donate_collateral(2).unwrap();
        assert_eq!(market.collateral, 7);
        assert_eq!(market.total_supply[..2], [5, 5]);

        let mut active_donation = [0; MAX_OUTCOMES];
        active_donation[..2].copy_from_slice(&[2, 1]);
        market
            .donate_internal_vector(&mut position, active_donation)
            .unwrap();
        assert_eq!(position.internal[..2], [3, 4]);
        assert_eq!(market.total_supply[..2], [3, 4]);
        assert_eq!(market.collateral, 7);
        market.resolve(1).unwrap();

        let mut resolved_donation = [0; MAX_OUTCOMES];
        resolved_donation[..2].copy_from_slice(&[1, 2]);
        market
            .donate_internal_vector(&mut position, resolved_donation)
            .unwrap();
        assert_eq!(position.internal[..2], [2, 2]);
        assert_eq!(market.total_supply[..2], [2, 2]);
        assert_eq!(market.collateral, 7);
        market.check_invariants().unwrap();

        for invalid in [
            [0; MAX_OUTCOMES],
            {
                let mut value = [0; MAX_OUTCOMES];
                value[0] = 3;
                value
            },
            {
                let mut value = [0; MAX_OUTCOMES];
                value[2] = 1;
                value
            },
        ] {
            let before = (market, position);
            assert!(market
                .donate_internal_vector(&mut position, invalid)
                .is_err());
            assert_eq!((market, position), before);
        }

        let mut full = MarketState::new(2, FinitePreset, binary_set(), u64::MAX).unwrap();
        let before = full;
        assert_eq!(full.donate_collateral(1), Err(Error::ArithmeticOverflow));
        assert_eq!(full, before);
        assert_eq!(full.donate_collateral(0), Err(Error::ZeroQuantity));
        assert_eq!(full, before);
    }

    fn fractional_market(mode: BasisMode) -> MarketState {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        vectors[0] = PayoutVector::new(7, weights_of(&[5, 2]));
        MarketState::new(2, mode, PayoutSet::new(1, 2, vectors), 0).unwrap()
    }

    #[test]
    fn aggregate_vector_redemption_is_exact_in_both_resolution_modes() {
        for mode in [FinitePreset, DerivedBasis] {
            let mut market = fractional_market(mode);
            let mut position = Position::EMPTY;
            market.split(&mut position, 14).unwrap();
            match mode {
                FinitePreset => market.resolve(0).unwrap(),
                DerivedBasis => market
                    .resolve_with_vector(PayoutVector::new(7, weights_of(&[5, 2])))
                    .unwrap(),
            }

            let mut inexact = [0; MAX_OUTCOMES];
            inexact[0] = 1;
            let before = (market, position);
            assert_eq!(
                market.redeem_internal_vector_exact(&mut position, inexact),
                Err(Error::RemainderRequired)
            );
            assert_eq!((market, position), before);

            let mut exact = [0; MAX_OUTCOMES];
            exact[..2].copy_from_slice(&[1, 1]);
            assert_eq!(
                market.redeem_internal_vector_exact(&mut position, exact),
                Ok(1)
            );
            assert_eq!(position.internal[..2], [13, 13]);
            assert_eq!(market.total_supply[..2], [13, 13]);
            assert_eq!(market.collateral, 13);
            market.check_invariants().unwrap();

            for invalid in [
                [0; MAX_OUTCOMES],
                {
                    let mut value = [0; MAX_OUTCOMES];
                    value[0] = 14;
                    value
                },
                {
                    let mut value = [0; MAX_OUTCOMES];
                    value[2] = 1;
                    value
                },
            ] {
                let before = (market, position);
                assert!(market
                    .redeem_internal_vector_exact(&mut position, invalid)
                    .is_err());
                assert_eq!((market, position), before);
            }
        }
    }

    #[test]
    fn aggregate_redemption_refuses_forged_insolvency_without_mutation() {
        let mut market = fractional_market(DerivedBasis);
        let mut position = Position::EMPTY;
        market.split(&mut position, 7).unwrap();
        market
            .resolve_with_vector(PayoutVector::new(7, weights_of(&[5, 2])))
            .unwrap();
        market.collateral = 0;
        let mut exact = [0; MAX_OUTCOMES];
        exact[..2].copy_from_slice(&[1, 1]);
        let before = (market, position);
        assert_eq!(
            market.redeem_internal_vector_exact(&mut position, exact),
            Err(Error::InvariantViolation)
        );
        assert_eq!((market, position), before);
    }
}
