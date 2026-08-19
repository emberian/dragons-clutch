#![forbid(unsafe_code)]

//! Adversarial state-machine model for claim-neutral resolution.
//!
//! The production adapter currently observes every canonical Token-2022
//! outcome mint during `Resolve`. This model separates two propositions that
//! are easy to conflate:
//!
//! 1. recording a payout fact is claim- and value-neutral; and
//! 2. observing all mints detects an impossible increase at that instant.
//!
//! Proposition 1 permits a smaller account plane on reachable states.
//! Proposition 2 cannot survive omission of the accounts that carry the fact.
//! Every payout-moving transition here therefore synchronizes the complete
//! mint vector even when resolution itself does not.

/// Bounded outcome width used by this executable model.
pub const MAX_OUTCOMES: usize = 4;
/// Bounded finite-payout width used by this executable model.
pub const MAX_PAYOUTS: usize = 4;

/// A deterministic refusal from the model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An outcome, payout, mode, or padding shape is invalid.
    Shape,
    /// Checked integer arithmetic failed.
    Arithmetic,
    /// The transition is unavailable in this lifecycle phase.
    Phase,
    /// A replay named a different immutable resolution fact.
    ResolutionConflict,
    /// A balance or collateral quantity is insufficient.
    Insufficient,
    /// A payout is not an exact integer number of collateral atoms.
    Remainder,
    /// SupplyLedger and kernel aggregate do not close.
    AggregateClosure,
    /// Current mint supply exceeds the last program-observed cache.
    ImpossibleMintIncrease,
    /// An exact-repeat gate found current mint supply unequal to its cache.
    MintCacheMismatch,
    /// Locked collateral is below the conservative required amount.
    Insolvent,
    /// Pooled Hoard custody does not equal locked backing, cash, and donation.
    HoardClosure,
}

/// Result alias for the model.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact integer payout weights over one common denominator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Payout {
    /// Positive common denominator.
    pub denominator: u64,
    /// Weights over the active outcome prefix; padding must be zero.
    pub weights: [u64; MAX_OUTCOMES],
}

impl Payout {
    /// All-zero sentinel, never an admitted payout.
    pub const ZERO: Self = Self {
        denominator: 0,
        weights: [0; MAX_OUTCOMES],
    };

    /// Construct a payout without hiding validation at the call site.
    pub const fn new(denominator: u64, weights: [u64; MAX_OUTCOMES]) -> Self {
        Self {
            denominator,
            weights,
        }
    }

    fn validate(self, outcomes: u8) -> Result<()> {
        if self.denominator == 0 || !(2..=MAX_OUTCOMES as u8).contains(&outcomes) {
            return Err(Error::Shape);
        }
        let mut sum = 0_u64;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let weight = self.weights[index];
            if index < usize::from(outcomes) {
                if weight > self.denominator {
                    return Err(Error::Shape);
                }
                sum = sum.checked_add(weight).ok_or(Error::Arithmetic)?;
            } else if weight != 0 {
                return Err(Error::Shape);
            }
            index += 1;
        }
        if sum != self.denominator {
            return Err(Error::Shape);
        }
        Ok(())
    }

    fn exact_payout(self, outcomes: u8, outcome: u8, quantity: u64) -> Result<u64> {
        self.validate(outcomes)?;
        if outcome >= outcomes || quantity == 0 {
            return Err(Error::Shape);
        }
        let numerator = u128::from(quantity)
            .checked_mul(u128::from(self.weights[usize::from(outcome)]))
            .ok_or(Error::Arithmetic)?;
        let denominator = u128::from(self.denominator);
        if !numerator.is_multiple_of(denominator) {
            return Err(Error::Remainder);
        }
        u64::try_from(numerator / denominator).map_err(|_| Error::Arithmetic)
    }

    fn liability_ceiling(self, outcomes: u8, total: &[u64; MAX_OUTCOMES]) -> Result<u64> {
        self.validate(outcomes)?;
        let mut numerator = 0_u128;
        let mut index = 0_usize;
        while index < usize::from(outcomes) {
            let term = u128::from(total[index])
                .checked_mul(u128::from(self.weights[index]))
                .ok_or(Error::Arithmetic)?;
            numerator = numerator.checked_add(term).ok_or(Error::Arithmetic)?;
            index += 1;
        }
        let denominator = u128::from(self.denominator);
        let quotient = numerator / denominator;
        let rounded = if numerator.is_multiple_of(denominator) {
            quotient
        } else {
            quotient.checked_add(1).ok_or(Error::Arithmetic)?
        };
        u64::try_from(rounded).map_err(|_| Error::Arithmetic)
    }
}

/// Immutable resolution seam selected by market terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BasisMode {
    /// Version-two resolution selects one frozen finite payout.
    Finite {
        /// Canonical candidate array.
        payouts: [Payout; MAX_PAYOUTS],
        /// Active candidate prefix.
        count: u8,
    },
    /// Version-three or version-four resolution installs a simplex vector.
    Derived {
        /// Frozen common denominator.
        denominator: u64,
    },
}

/// Exact persisted resolution fact, including evidence identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionFact {
    /// Categorical v2 record.
    V2 {
        /// Canonical source/window fact identity.
        evidence: u64,
        /// Selected frozen payout index.
        payout_index: u8,
    },
    /// Native point v3 record.
    V3 {
        /// Canonical source/window fact identity.
        evidence: u64,
        /// Exact record-owned vector.
        vector: Payout,
    },
    /// Native occupation v4 record.
    V4 {
        /// Canonical archive/statistic fact identity.
        evidence: u64,
        /// Exact record-owned vector.
        vector: Payout,
    },
}

/// Market lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// Claims can still be split or bridged.
    Active,
    /// One immutable resolution fact has been recorded.
    Resolved,
}

/// Which mint supplies a resolution attempt observes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolveObservation {
    /// Current production first-resolution behavior: observe all mints.
    Full,
    /// Proposed claim-neutral behavior: observe no mints.
    None,
    /// Research-only counterexample: observe an arbitrary subset.
    Partial([bool; MAX_OUTCOMES]),
}

/// Pooled market state relevant to claim-neutrality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct State {
    /// Active outcome prefix.
    pub outcomes: u8,
    /// Immutable basis mode.
    pub mode: BasisMode,
    /// Lifecycle phase.
    pub phase: Phase,
    /// Sole persisted resolution owner.
    pub resolution: Option<ResolutionFact>,
    /// Program-owned internal aggregate.
    pub internal: [u64; MAX_OUTCOMES],
    /// Last observed Token-2022 mint supply.
    pub cached_external: [u64; MAX_OUTCOMES],
    /// Authoritative current Token-2022 mint supply.
    pub actual_external: [u64; MAX_OUTCOMES],
    /// Kernel's conservative total supply.
    pub kernel_total: [u64; MAX_OUTCOMES],
    /// Retained claim backing.
    pub locked_backing: u64,
    /// Aggregate Position cash, modeled market-wide.
    pub position_cash: u64,
    /// Actual collateral tokens in pooled custody.
    pub hoard_tokens: u64,
    /// Unowned direct-deposit surplus.
    pub direct_surplus: u64,
}

impl State {
    /// Create a blank market in one immutable mode.
    pub fn blank(outcomes: u8, mode: BasisMode) -> Result<Self> {
        let state = Self {
            outcomes,
            mode,
            phase: Phase::Active,
            resolution: None,
            internal: [0; MAX_OUTCOMES],
            cached_external: [0; MAX_OUTCOMES],
            actual_external: [0; MAX_OUTCOMES],
            kernel_total: [0; MAX_OUTCOMES],
            locked_backing: 0,
            position_cash: 0,
            hoard_tokens: 0,
            direct_surplus: 0,
        };
        state.check_reachable_invariants()?;
        Ok(state)
    }

    /// Facts a mint-free Resolve can decide from persisted program state.
    pub fn check_cached_invariants(&self) -> Result<()> {
        self.validate_mode()?;
        let count = usize::from(self.outcomes);
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            if index < count {
                let closed = self.internal[index]
                    .checked_add(self.cached_external[index])
                    .ok_or(Error::Arithmetic)?;
                if closed != self.kernel_total[index] {
                    return Err(Error::AggregateClosure);
                }
            } else if self.internal[index] != 0
                || self.cached_external[index] != 0
                || self.kernel_total[index] != 0
            {
                return Err(Error::Shape);
            }
            index += 1;
        }
        let custody = self
            .locked_backing
            .checked_add(self.position_cash)
            .and_then(|value| value.checked_add(self.direct_surplus))
            .ok_or(Error::Arithmetic)?;
        if custody != self.hoard_tokens {
            return Err(Error::HoardClosure);
        }
        let required = self.required_collateral()?;
        if self.locked_backing < required {
            return Err(Error::Insolvent);
        }
        Ok(())
    }

    /// Reachability invariant additionally owned by the canonical mint plane.
    pub fn check_reachable_invariants(&self) -> Result<()> {
        self.check_cached_invariants()?;
        let active = usize::from(self.outcomes);
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            if index < active {
                if self.actual_external[index] > self.cached_external[index] {
                    return Err(Error::ImpossibleMintIncrease);
                }
            } else if self.actual_external[index] != 0 {
                return Err(Error::Shape);
            }
            index += 1;
        }
        Ok(())
    }

    /// Deposit pooled collateral and credit Position cash.
    pub fn endow(&mut self, quantity: u64) -> Result<()> {
        self.transact(|next| {
            if quantity == 0 {
                return Err(Error::Shape);
            }
            next.hoard_tokens = next
                .hoard_tokens
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.position_cash = next
                .position_cash
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.check_reachable_invariants()
        })
    }

    /// Convert Position cash into one complete internal set.
    pub fn split(&mut self, quantity: u64) -> Result<()> {
        self.transact(|next| {
            next.synchronize_full()?;
            next.require_active()?;
            if quantity == 0 || next.position_cash < quantity {
                return Err(Error::Insufficient);
            }
            let mut index = 0_usize;
            while index < usize::from(next.outcomes) {
                next.internal[index] = next.internal[index]
                    .checked_add(quantity)
                    .ok_or(Error::Arithmetic)?;
                next.kernel_total[index] = next.kernel_total[index]
                    .checked_add(quantity)
                    .ok_or(Error::Arithmetic)?;
                index += 1;
            }
            next.position_cash -= quantity;
            next.locked_backing = next
                .locked_backing
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.check_reachable_invariants()
        })
    }

    /// Move one active claim from internal to bearer form.
    pub fn materialize(&mut self, outcome: u8, quantity: u64) -> Result<()> {
        self.transact(|next| {
            next.synchronize_full()?;
            next.require_active()?;
            let index = next.outcome_index(outcome, quantity)?;
            if next.internal[index] < quantity {
                return Err(Error::Insufficient);
            }
            next.internal[index] -= quantity;
            next.actual_external[index] = next.actual_external[index]
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.cached_external[index] = next.cached_external[index]
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.check_reachable_invariants()
        })
    }

    /// Move one active bearer claim back into internal form.
    pub fn dematerialize(&mut self, outcome: u8, quantity: u64) -> Result<()> {
        self.transact(|next| {
            next.synchronize_full()?;
            next.require_active()?;
            let index = next.outcome_index(outcome, quantity)?;
            if next.actual_external[index] < quantity {
                return Err(Error::Insufficient);
            }
            next.actual_external[index] -= quantity;
            next.cached_external[index] -= quantity;
            next.internal[index] = next.internal[index]
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.check_reachable_invariants()
        })
    }

    /// Ordinary holder burn: lower actual supply without touching program state.
    pub fn direct_burn(&mut self, outcome: u8, quantity: u64) -> Result<()> {
        self.transact(|next| {
            next.check_reachable_invariants()?;
            let index = next.outcome_index(outcome, quantity)?;
            if next.actual_external[index] < quantity {
                return Err(Error::Insufficient);
            }
            next.actual_external[index] -= quantity;
            next.check_reachable_invariants()
        })
    }

    /// Fault injection unavailable to a correctly authorized public history.
    pub fn inject_unaccounted_mint(&mut self, outcome: u8, quantity: u64) -> Result<()> {
        let index = self.outcome_index(outcome, quantity)?;
        self.actual_external[index] = self.actual_external[index]
            .checked_add(quantity)
            .ok_or(Error::Arithmetic)?;
        Ok(())
    }

    /// Unsolicited collateral transfer, which creates no owned balance.
    pub fn donate_to_hoard(&mut self, quantity: u64) -> Result<()> {
        self.transact(|next| {
            if quantity == 0 {
                return Err(Error::Shape);
            }
            next.hoard_tokens = next
                .hoard_tokens
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.direct_surplus = next
                .direct_surplus
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.check_reachable_invariants()
        })
    }

    /// Model current Resolve: full synchronization on the first record.
    ///
    /// An exact repeat requires the mint cache already equal current truth,
    /// matching the production handler's idempotent-repeat branch.
    pub fn resolve_current_full(&mut self, fact: ResolutionFact) -> Result<()> {
        self.transact(|next| {
            if next.phase == Phase::Resolved {
                next.require_same_resolution(fact)?;
                let mut index = 0_usize;
                while index < usize::from(next.outcomes) {
                    if next.actual_external[index] != next.cached_external[index] {
                        return Err(Error::MintCacheMismatch);
                    }
                    index += 1;
                }
                return Ok(());
            }
            next.synchronize_full()?;
            next.record_resolution(fact)
        })
    }

    /// Proposed claim-neutral Resolve: no outcome mint is consulted or changed.
    pub fn resolve_claim_neutral(&mut self, fact: ResolutionFact) -> Result<()> {
        self.transact(|next| {
            next.check_cached_invariants()?;
            next.record_resolution(fact)
        })
    }

    /// Research-only partial observation, demonstrating its exact boundary.
    pub fn resolve_with_observation(
        &mut self,
        fact: ResolutionFact,
        observation: ResolveObservation,
    ) -> Result<()> {
        match observation {
            ResolveObservation::Full => self.resolve_current_full(fact),
            ResolveObservation::None => self.resolve_claim_neutral(fact),
            ResolveObservation::Partial(mask) => self.transact(|next| {
                next.synchronize_mask(mask)?;
                next.record_resolution(fact)
            }),
        }
    }

    /// Redeem an internal claim after fully synchronizing current mint truth.
    pub fn redeem_internal(&mut self, outcome: u8, quantity: u64) -> Result<u64> {
        self.transact(|next| {
            next.synchronize_full()?;
            let payout = next
                .resolved_payout()?
                .exact_payout(next.outcomes, outcome, quantity)?;
            let index = next.outcome_index(outcome, quantity)?;
            if next.internal[index] < quantity || next.locked_backing < payout {
                return Err(Error::Insufficient);
            }
            next.internal[index] -= quantity;
            next.kernel_total[index] -= quantity;
            next.locked_backing -= payout;
            next.position_cash = next
                .position_cash
                .checked_add(payout)
                .ok_or(Error::Arithmetic)?;
            next.check_reachable_invariants()?;
            Ok(payout)
        })
    }

    /// Burn and redeem a bearer claim after full current-mint synchronization.
    pub fn redeem_external(&mut self, outcome: u8, quantity: u64) -> Result<u64> {
        self.transact(|next| {
            next.synchronize_full()?;
            let payout = next
                .resolved_payout()?
                .exact_payout(next.outcomes, outcome, quantity)?;
            let index = next.outcome_index(outcome, quantity)?;
            if next.actual_external[index] < quantity
                || next.locked_backing < payout
                || next.hoard_tokens < payout
            {
                return Err(Error::Insufficient);
            }
            next.actual_external[index] -= quantity;
            next.cached_external[index] -= quantity;
            next.kernel_total[index] -= quantity;
            next.locked_backing -= payout;
            next.hoard_tokens -= payout;
            next.check_reachable_invariants()?;
            Ok(payout)
        })
    }

    fn validate_mode(&self) -> Result<()> {
        if !(2..=MAX_OUTCOMES as u8).contains(&self.outcomes) {
            return Err(Error::Shape);
        }
        match self.mode {
            BasisMode::Finite { payouts, count } => {
                if count == 0 || usize::from(count) > MAX_PAYOUTS {
                    return Err(Error::Shape);
                }
                let mut index = 0_usize;
                while index < MAX_PAYOUTS {
                    if index < usize::from(count) {
                        payouts[index].validate(self.outcomes)?;
                    } else if payouts[index] != Payout::ZERO {
                        return Err(Error::Shape);
                    }
                    index += 1;
                }
            }
            BasisMode::Derived { denominator } => {
                if denominator == 0 {
                    return Err(Error::Shape);
                }
            }
        }
        Ok(())
    }

    fn payout_for_fact(&self, fact: ResolutionFact) -> Result<Payout> {
        match (self.mode, fact) {
            (BasisMode::Finite { payouts, count }, ResolutionFact::V2 { payout_index, .. })
                if payout_index < count =>
            {
                Ok(payouts[usize::from(payout_index)])
            }
            (
                BasisMode::Derived { denominator },
                ResolutionFact::V3 { vector, .. } | ResolutionFact::V4 { vector, .. },
            ) if vector.denominator == denominator => {
                vector.validate(self.outcomes)?;
                Ok(vector)
            }
            _ => Err(Error::Shape),
        }
    }

    fn resolved_payout(&self) -> Result<Payout> {
        if self.phase != Phase::Resolved {
            return Err(Error::Phase);
        }
        self.payout_for_fact(self.resolution.ok_or(Error::Phase)?)
    }

    fn required_collateral(&self) -> Result<u64> {
        match self.phase {
            Phase::Active => match self.mode {
                BasisMode::Finite { payouts, count } => {
                    let mut required = 0_u64;
                    let mut index = 0_usize;
                    while index < usize::from(count) {
                        let candidate =
                            payouts[index].liability_ceiling(self.outcomes, &self.kernel_total)?;
                        required = required.max(candidate);
                        index += 1;
                    }
                    Ok(required)
                }
                BasisMode::Derived { .. } => Ok(self.kernel_total[..usize::from(self.outcomes)]
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(0)),
            },
            Phase::Resolved => self
                .resolved_payout()?
                .liability_ceiling(self.outcomes, &self.kernel_total),
        }
    }

    fn synchronize_full(&mut self) -> Result<()> {
        self.synchronize_mask([true; MAX_OUTCOMES])
    }

    fn synchronize_mask(&mut self, mask: [bool; MAX_OUTCOMES]) -> Result<()> {
        self.check_cached_invariants()?;
        let mut next_cache = self.cached_external;
        let mut next_total = self.kernel_total;
        let mut index = 0_usize;
        while index < usize::from(self.outcomes) {
            if mask[index] {
                if self.actual_external[index] > self.cached_external[index] {
                    return Err(Error::ImpossibleMintIncrease);
                }
                next_cache[index] = self.actual_external[index];
                next_total[index] = self.internal[index]
                    .checked_add(self.actual_external[index])
                    .ok_or(Error::Arithmetic)?;
            }
            index += 1;
        }
        self.cached_external = next_cache;
        self.kernel_total = next_total;
        self.check_cached_invariants()
    }

    fn record_resolution(&mut self, fact: ResolutionFact) -> Result<()> {
        let payout = self.payout_for_fact(fact)?;
        match self.phase {
            Phase::Active => {
                let required = payout.liability_ceiling(self.outcomes, &self.kernel_total)?;
                if required > self.locked_backing {
                    return Err(Error::Insolvent);
                }
                self.phase = Phase::Resolved;
                self.resolution = Some(fact);
                self.check_cached_invariants()
            }
            Phase::Resolved => self.require_same_resolution(fact),
        }
    }

    fn require_same_resolution(&self, fact: ResolutionFact) -> Result<()> {
        if self.resolution == Some(fact) {
            Ok(())
        } else {
            Err(Error::ResolutionConflict)
        }
    }

    fn require_active(&self) -> Result<()> {
        if self.phase == Phase::Active {
            Ok(())
        } else {
            Err(Error::Phase)
        }
    }

    fn outcome_index(&self, outcome: u8, quantity: u64) -> Result<usize> {
        if outcome >= self.outcomes || quantity == 0 {
            Err(Error::Shape)
        } else {
            Ok(usize::from(outcome))
        }
    }

    fn transact<T, F>(&mut self, transition: F) -> Result<T>
    where
        F: FnOnce(&mut Self) -> Result<T>,
    {
        let mut next = *self;
        match transition(&mut next) {
            Ok(value) => {
                *self = next;
                Ok(value)
            }
            Err(error) => Err(error),
        }
    }
}

/// Exact structural work visible in the current and proposed account planes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolveFootprint {
    /// Total instruction accounts for the selected point/occupation shape.
    pub account_count: usize,
    /// Canonical outcome-mint accounts.
    pub outcome_mint_accounts: usize,
    /// Outcome-mint PDA derivations (pre and post in the current handler).
    pub outcome_mint_pda_derivations: usize,
    /// Mint policy admissions (pre and post in the current handler).
    pub outcome_mint_admissions: usize,
    /// Whole-vector semantic loops outside mint decoding.
    pub vector_loops: usize,
    /// Whether Resolve writes the SupplyLedger solely to synchronize burns.
    pub synchronization_writes_supply: bool,
}

impl ResolveFootprint {
    /// Current exact account and structural-loop shape.
    pub fn current(outcomes: u8, occupation: bool) -> Result<Self> {
        if !(2..=16).contains(&outcomes) {
            return Err(Error::Shape);
        }
        let count = usize::from(outcomes);
        let prefix = if occupation { 10 } else { 11 };
        Ok(Self {
            account_count: prefix + count,
            outcome_mint_accounts: count,
            outcome_mint_pda_derivations: 2 * count,
            outcome_mint_admissions: 2 * count,
            vector_loops: 2,
            synchronization_writes_supply: true,
        })
    }

    /// Claim-neutral shape retaining SupplyLedger as a read-only closure fact.
    pub fn claim_neutral(outcomes: u8, occupation: bool) -> Result<Self> {
        if !(2..=16).contains(&outcomes) {
            return Err(Error::Shape);
        }
        Ok(Self {
            account_count: if occupation { 10 } else { 11 },
            outcome_mint_accounts: 0,
            outcome_mint_pda_derivations: 0,
            outcome_mint_admissions: 0,
            vector_loops: 0,
            synchronization_writes_supply: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_hot(index: usize) -> Payout {
        let mut weights = [0; MAX_OUTCOMES];
        weights[index] = 1;
        Payout::new(1, weights)
    }

    fn finite_mode() -> BasisMode {
        BasisMode::Finite {
            payouts: [one_hot(0), one_hot(1), Payout::ZERO, Payout::ZERO],
            count: 2,
        }
    }

    fn derived_mode() -> BasisMode {
        BasisMode::Derived { denominator: 4 }
    }

    fn v2(evidence: u64, payout_index: u8) -> ResolutionFact {
        ResolutionFact::V2 {
            evidence,
            payout_index,
        }
    }

    fn v3(evidence: u64) -> ResolutionFact {
        ResolutionFact::V3 {
            evidence,
            vector: Payout::new(4, [1, 3, 0, 0]),
        }
    }

    fn v4(evidence: u64) -> ResolutionFact {
        ResolutionFact::V4 {
            evidence,
            vector: Payout::new(4, [2, 2, 0, 0]),
        }
    }

    fn funded(mode: BasisMode, quantity: u64) -> State {
        let mut state = State::blank(2, mode).unwrap();
        state.endow(quantity).unwrap();
        state.split(quantity).unwrap();
        state
    }

    #[test]
    fn stale_cache_overestimate_is_safe_and_next_consumer_converges() {
        let mut initial = funded(finite_mode(), 10);
        initial.materialize(0, 6).unwrap();
        initial.direct_burn(0, 2).unwrap();

        let mut current = initial;
        let mut neutral = initial;
        current.resolve_current_full(v2(7, 0)).unwrap();
        neutral.resolve_claim_neutral(v2(7, 0)).unwrap();
        assert_eq!(current.cached_external[0], 4);
        assert_eq!(neutral.cached_external[0], 6);
        assert_eq!(neutral.actual_external[0], 4);

        assert_eq!(current.redeem_internal(0, 2), Ok(2));
        assert_eq!(neutral.redeem_internal(0, 2), Ok(2));
        assert_eq!(neutral, current);
    }

    #[test]
    fn burn_after_resolve_changes_current_repeat_but_not_resolution_conflict() {
        let mut current = funded(finite_mode(), 8);
        current.materialize(0, 4).unwrap();
        current.resolve_current_full(v2(11, 0)).unwrap();
        let mut neutral = current;
        current.direct_burn(0, 1).unwrap();
        neutral.direct_burn(0, 1).unwrap();

        let before = current;
        assert_eq!(
            current.resolve_current_full(v2(11, 0)),
            Err(Error::MintCacheMismatch)
        );
        assert_eq!(current, before);
        assert_eq!(neutral.resolve_claim_neutral(v2(11, 0)), Ok(()));
        assert_eq!(
            neutral.resolve_claim_neutral(v2(12, 0)),
            Err(Error::ResolutionConflict)
        );
        assert_eq!(neutral.redeem_external(0, 1), Ok(1));
    }

    #[test]
    fn impossible_mint_increase_is_the_minimal_information_counterexample() {
        let mut full = funded(finite_mode(), 5);
        full.materialize(0, 2).unwrap();
        let mut neutral = full;
        full.inject_unaccounted_mint(0, 1).unwrap();
        neutral.inject_unaccounted_mint(0, 1).unwrap();
        assert_eq!(full.check_cached_invariants(), Ok(()));
        assert_eq!(
            full.check_reachable_invariants(),
            Err(Error::ImpossibleMintIncrease)
        );

        let before = full;
        assert_eq!(
            full.resolve_current_full(v2(1, 0)),
            Err(Error::ImpossibleMintIncrease)
        );
        assert_eq!(full, before);
        assert_eq!(neutral.resolve_claim_neutral(v2(1, 0)), Ok(()));

        // No payout-moving consumer trusts the recorded fact alone. The first
        // such consumer re-reads all mints and refuses atomically.
        let resolved = neutral;
        assert_eq!(
            neutral.redeem_external(0, 1),
            Err(Error::ImpossibleMintIncrease)
        );
        assert_eq!(neutral, resolved);
    }

    #[test]
    fn partial_observation_only_proves_the_indices_it_reads() {
        let mut partial = funded(finite_mode(), 6);
        partial.materialize(0, 2).unwrap();
        partial.materialize(1, 2).unwrap();
        partial.inject_unaccounted_mint(1, 1).unwrap();
        let before = partial;
        assert_eq!(
            partial.resolve_with_observation(
                v2(3, 0),
                ResolveObservation::Partial([true, false, false, false])
            ),
            Ok(())
        );
        assert_eq!(partial.actual_external[1], 3);
        assert_eq!(partial.cached_external[1], 2);
        assert_eq!(
            partial.redeem_internal(0, 1),
            Err(Error::ImpossibleMintIncrease)
        );

        let mut observed_fault = before;
        assert_eq!(
            observed_fault.resolve_with_observation(
                v2(3, 0),
                ResolveObservation::Partial([false, true, false, false])
            ),
            Err(Error::ImpossibleMintIncrease)
        );
        assert_eq!(observed_fault, before);
    }

    #[test]
    fn all_three_resolution_abis_are_claim_neutral_and_conflict_exactly() {
        let mut categorical = funded(finite_mode(), 4);
        categorical.resolve_claim_neutral(v2(20, 1)).unwrap();
        assert_eq!(categorical.resolve_claim_neutral(v2(20, 1)), Ok(()));
        assert_eq!(
            categorical.resolve_claim_neutral(v2(20, 0)),
            Err(Error::ResolutionConflict)
        );

        let mut point = funded(derived_mode(), 8);
        point.resolve_claim_neutral(v3(21)).unwrap();
        assert_eq!(point.resolve_claim_neutral(v3(21)), Ok(()));
        assert_eq!(
            point.resolve_claim_neutral(v4(21)),
            Err(Error::ResolutionConflict)
        );

        let mut occupation = funded(derived_mode(), 8);
        occupation.resolve_claim_neutral(v4(22)).unwrap();
        assert_eq!(occupation.resolve_claim_neutral(v4(22)), Ok(()));
        assert_eq!(
            occupation.resolve_claim_neutral(v4(23)),
            Err(Error::ResolutionConflict)
        );
    }

    #[test]
    fn materialize_and_dematerialize_are_active_only_and_supply_neutral() {
        let mut state = funded(derived_mode(), 8);
        state.materialize(0, 4).unwrap();
        assert_eq!(state.kernel_total, [8, 8, 0, 0]);
        state.direct_burn(0, 1).unwrap();
        state.dematerialize(0, 2).unwrap();
        assert_eq!(state.actual_external[0], 1);
        assert_eq!(state.cached_external[0], 1);
        assert_eq!(state.kernel_total, [7, 8, 0, 0]);
        state.resolve_claim_neutral(v3(30)).unwrap();
        let before = state;
        assert_eq!(state.materialize(0, 1), Err(Error::Phase));
        assert_eq!(state, before);
        assert_eq!(state.dematerialize(0, 1), Err(Error::Phase));
        assert_eq!(state, before);
    }

    #[test]
    fn internal_external_redemption_and_donations_preserve_pooled_hoard() {
        let mut state = funded(derived_mode(), 8);
        state.materialize(0, 4).unwrap();
        state.donate_to_hoard(7).unwrap();
        state.direct_burn(0, 1).unwrap();
        let hoard_after_donation = state.hoard_tokens;
        state.resolve_claim_neutral(v4(40)).unwrap();
        assert_eq!(state.redeem_internal(1, 2), Ok(1));
        assert_eq!(state.hoard_tokens, hoard_after_donation);
        assert_eq!(state.position_cash, 1);
        assert_eq!(state.redeem_external(0, 2), Ok(1));
        assert_eq!(state.direct_surplus, 7);
        assert_eq!(state.hoard_tokens, hoard_after_donation - 1);
        state.check_reachable_invariants().unwrap();
    }

    #[test]
    fn overflow_and_impossible_increase_refuse_atomically_at_their_first_available_gate() {
        let mut overflowing_cache = State::blank(2, finite_mode()).unwrap();
        overflowing_cache.internal[0] = u64::MAX;
        overflowing_cache.cached_external[0] = 1;
        overflowing_cache.actual_external[0] = 1;
        overflowing_cache.kernel_total[0] = u64::MAX;
        overflowing_cache.locked_backing = u64::MAX;
        overflowing_cache.hoard_tokens = u64::MAX;
        let malformed = overflowing_cache;
        assert_eq!(
            overflowing_cache.resolve_claim_neutral(v2(50, 0)),
            Err(Error::Arithmetic)
        );
        assert_eq!(overflowing_cache, malformed);

        // With a representable cached closure, I + actual cannot overflow
        // while actual <= cache. This fault is therefore classified earlier
        // and more precisely as an impossible increase, not arithmetic.
        let mut increased = State::blank(2, finite_mode()).unwrap();
        increased.internal[0] = u64::MAX;
        increased.kernel_total[0] = u64::MAX;
        increased.locked_backing = u64::MAX;
        increased.hoard_tokens = u64::MAX;
        increased.inject_unaccounted_mint(0, 1).unwrap();
        increased.check_cached_invariants().unwrap();
        increased.resolve_claim_neutral(v2(51, 0)).unwrap();
        let resolved = increased;
        assert_eq!(
            increased.redeem_internal(0, 1),
            Err(Error::ImpossibleMintIncrease)
        );
        assert_eq!(increased, resolved);
    }

    #[derive(Clone, Copy)]
    enum Action {
        Burn(u8, u64),
        Materialize(u8, u64),
        Dematerialize(u8, u64),
        Resolve,
        RedeemInternal(u8, u64),
        RedeemExternal(u8, u64),
        Donate(u64),
    }

    fn apply_action(state: &mut State, action: Action) -> Result<()> {
        match action {
            Action::Burn(outcome, quantity) => state.direct_burn(outcome, quantity),
            Action::Materialize(outcome, quantity) => state.materialize(outcome, quantity),
            Action::Dematerialize(outcome, quantity) => state.dematerialize(outcome, quantity),
            Action::Resolve => state.resolve_claim_neutral(v2(77, 0)),
            Action::RedeemInternal(outcome, quantity) => {
                state.redeem_internal(outcome, quantity).map(|_| ())
            }
            Action::RedeemExternal(outcome, quantity) => {
                state.redeem_external(outcome, quantity).map(|_| ())
            }
            Action::Donate(quantity) => state.donate_to_hoard(quantity),
        }
    }

    fn explore(state: State, actions: &[Action], depth: usize, visited: &mut u64) {
        *visited += 1;
        state.check_reachable_invariants().unwrap();
        if depth == 0 {
            return;
        }
        for action in actions {
            let mut next = state;
            let before = next;
            match apply_action(&mut next, *action) {
                Ok(()) => {
                    next.check_reachable_invariants().unwrap();
                    explore(next, actions, depth - 1, visited);
                }
                Err(_) => assert_eq!(next, before, "refusal must be atomic"),
            }
        }
    }

    #[test]
    fn bounded_state_machine_preserves_reachability_through_arbitrary_burns() {
        let mut initial = funded(finite_mode(), 3);
        initial.materialize(0, 2).unwrap();
        initial.materialize(1, 1).unwrap();
        let actions = [
            Action::Burn(0, 1),
            Action::Burn(1, 1),
            Action::Materialize(0, 1),
            Action::Dematerialize(0, 1),
            Action::Resolve,
            Action::RedeemInternal(0, 1),
            Action::RedeemExternal(0, 1),
            Action::Donate(1),
        ];
        let mut visited = 0_u64;
        explore(initial, &actions, 5, &mut visited);
        assert!(visited > 1_000);
    }

    #[test]
    fn maximum_width_footprint_removes_sixteen_accounts_and_thirty_two_admissions() {
        let current_point = ResolveFootprint::current(16, false).unwrap();
        let neutral_point = ResolveFootprint::claim_neutral(16, false).unwrap();
        assert_eq!(current_point.account_count, 27);
        assert_eq!(neutral_point.account_count, 11);
        assert_eq!(current_point.outcome_mint_accounts, 16);
        assert_eq!(current_point.outcome_mint_pda_derivations, 32);
        assert_eq!(current_point.outcome_mint_admissions, 32);
        assert_eq!(neutral_point.outcome_mint_admissions, 0);

        let current_occupation = ResolveFootprint::current(16, true).unwrap();
        let neutral_occupation = ResolveFootprint::claim_neutral(16, true).unwrap();
        assert_eq!(current_occupation.account_count, 26);
        assert_eq!(neutral_occupation.account_count, 10);
        assert!(current_occupation.synchronization_writes_supply);
        assert!(!neutral_occupation.synchronization_writes_supply);
    }
}
