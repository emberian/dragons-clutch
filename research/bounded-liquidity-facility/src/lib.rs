#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Exact, allocation-free bounded quadratic liquidity-facility model.
//!
//! This crate models one fully capitalized market-making facility over a
//! Dragon's Clutch native Egg basis. It owns no Solana account, token, source,
//! clock, or call-auction authority. The live adapter boundary is documented in
//! the crate README and the companion design document.

use core::convert::TryFrom;

/// Maximum active native Eggs in one modeled facility.
pub const MAX_OUTCOMES: usize = 16;
/// Largest admitted collateral, Egg inventory, depth, or sponsor-capital value.
pub const MAX_ATOMS: u64 = 1_000_000_000_000;
/// Largest admitted denominator for the exact initial-price simplex.
pub const MAX_PRICE_DENOMINATOR: u64 = 1_000_000_000;

/// Fixed-width external identity or authenticated digest.
pub type Id = [u8; 32];

/// Checked refusal from a model transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A required identity is all zero or two distinct identities coincide.
    InvalidIdentity,
    /// The outcome width or fixed-capacity padding is invalid.
    InvalidBasis,
    /// A required amount, denominator, or duration is zero.
    ZeroValue,
    /// A value exceeds the frozen arithmetic domain.
    ParameterOutOfRange,
    /// Checked arithmetic overflowed.
    ArithmeticOverflow,
    /// The immutable schedule is unordered or a transition uses the wrong slot.
    InvalidSchedule,
    /// A request contains noncanonical padding or both flow directions on one Egg.
    NonCanonicalFlow,
    /// Egg flow is not a multiple of the conservative universal settlement lot.
    NonIntegralLot,
    /// A buyback exceeds the facility-attributed outstanding Egg inventory.
    InsufficientInventory,
    /// A post-trade inventory coordinate exceeds its immutable cap.
    InventoryLimit,
    /// The post-trade inventory leaves the nonnegative quadratic-price domain.
    PriceDomain,
    /// The sponsor deposit is below the exact pre-trade loss capitalization.
    InsufficientCapital,
    /// The facility cannot fund the staged cash/Hoard transition.
    InsufficientCash,
    /// A transition is not admitted in the current lifecycle phase.
    InvalidPhase,
    /// The caller binding differs from the immutable sponsor.
    MismatchedSponsor,
    /// A payout vector is not an exact nonnegative integer simplex.
    InvalidPayoutVector,
    /// The initial price vector is not an exact nonnegative integer simplex.
    InvalidPriceVector,
    /// A cached field disagrees with the exact potential/backing identities.
    InvariantViolation,
}

/// Result alias for total checked operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Immutable bounded quadratic facility policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FacilityPolicyV1 {
    /// Digest of the complete canonical policy bytes.
    pub policy_id: Id,
    /// Exact base Market identity.
    pub market: Id,
    /// Digest of the immutable native Terms bytes.
    pub terms_digest: Id,
    /// Deterministic recurring Instance identity.
    pub instance_id: Id,
    /// Domain binding shared by base coefficient claims and admitted wrappers.
    pub claim_domain_digest: Id,
    /// Number of active native Eggs.
    pub outcome_count: u8,
    /// Exact native payout denominator.
    pub payout_denominator: u64,
    /// Exact denominator shared by the initial-price weights.
    pub initial_price_denominator: u64,
    /// Initial-price numerators followed by canonical zero padding.
    pub initial_price_weights: [u64; MAX_OUTCOMES],
    /// Quadratic depth `b`, in raw Egg atoms.
    pub depth_atoms: u64,
    /// Maximum facility-attributed external Egg inventory per outcome.
    pub max_inventory: [u64; MAX_OUTCOMES],
    /// First slot in which ordinary two-sided facility transitions are admitted.
    pub trading_open_slot: u64,
    /// First slot in which ordinary sells stop and permissionless unwind may begin.
    pub trading_close_slot: u64,
    /// First slot in which authenticated resolution may consume facility holdings.
    pub maturity_slot: u64,
}

impl FacilityPolicyV1 {
    /// Validate immutable identities, bounds, schedule, and conservative lots.
    pub fn validate(&self) -> Result<()> {
        check_id(self.policy_id)?;
        check_id(self.market)?;
        check_id(self.terms_digest)?;
        check_id(self.instance_id)?;
        check_id(self.claim_domain_digest)?;
        if self.outcome_count < 2 || usize::from(self.outcome_count) > MAX_OUTCOMES {
            return Err(Error::InvalidBasis);
        }
        if self.payout_denominator == 0
            || self.initial_price_denominator == 0
            || self.depth_atoms == 0
        {
            return Err(Error::ZeroValue);
        }
        if self.payout_denominator > MAX_ATOMS
            || self.initial_price_denominator > MAX_PRICE_DENOMINATOR
            || self.depth_atoms > MAX_ATOMS
        {
            return Err(Error::ParameterOutOfRange);
        }
        if self.trading_open_slot >= self.trading_close_slot
            || self.trading_close_slot >= self.maturity_slot
        {
            return Err(Error::InvalidSchedule);
        }
        validate_padding(self.outcome_count, &self.max_inventory)?;
        let mut any = false;
        let mut i = 0usize;
        while i < usize::from(self.outcome_count) {
            let cap = self.max_inventory[i];
            if cap > MAX_ATOMS {
                return Err(Error::ParameterOutOfRange);
            }
            if !cap.is_multiple_of(self.payout_denominator) {
                return Err(Error::NonIntegralLot);
            }
            any |= cap != 0;
            i += 1;
        }
        if !any {
            return Err(Error::ZeroValue);
        }
        validate_initial_price(self)?;
        Ok(())
    }

    /// Conservative integer sponsor capital covering the global quadratic loss bound.
    ///
    /// For initial simplex price `pi`, the rational bound is
    /// `b/2 * max_j ||e_j - pi||^2`. This function takes the ceiling once at
    /// policy admission. Runtime trade accounting uses the separate canonical
    /// rounded potential.
    pub fn minimum_sponsor_capital(&self) -> Result<u64> {
        self.validate()?;
        let denominator = u128::from(self.initial_price_denominator);
        let mut sum_squares = 0u128;
        let mut i = 0usize;
        while i < usize::from(self.outcome_count) {
            let weight = u128::from(self.initial_price_weights[i]);
            sum_squares = sum_squares
                .checked_add(
                    weight
                        .checked_mul(weight)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            i += 1;
        }
        let denominator_squared = denominator
            .checked_mul(denominator)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut maximum_norm_numerator = 0u128;
        i = 0;
        while i < usize::from(self.outcome_count) {
            let twice_selected = denominator
                .checked_mul(u128::from(self.initial_price_weights[i]))
                .and_then(|value| value.checked_mul(2))
                .ok_or(Error::ArithmeticOverflow)?;
            let norm_numerator = denominator_squared
                .checked_add(sum_squares)
                .and_then(|value| value.checked_sub(twice_selected))
                .ok_or(Error::InvariantViolation)?;
            if norm_numerator > maximum_norm_numerator {
                maximum_norm_numerator = norm_numerator;
            }
            i += 1;
        }
        let numerator = u128::from(self.depth_atoms)
            .checked_mul(maximum_norm_numerator)
            .ok_or(Error::ArithmeticOverflow)?;
        let loss_denominator = denominator_squared
            .checked_mul(2)
            .ok_or(Error::ArithmeticOverflow)?;
        let value = ceil_div(numerator, loss_denominator)?;
        u64::try_from(value).map_err(|_| Error::ArithmeticOverflow)
    }
}

/// Exact rational instantaneous price vector of the unrounded quadratic potential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceVectorV1 {
    /// Active numerators followed by canonical zero padding.
    pub numerators: [u128; MAX_OUTCOMES],
    /// Positive common denominator `initial_price_denominator*b*n`.
    pub denominator: u128,
    /// Active width.
    pub outcome_count: u8,
}

impl PriceVectorV1 {
    /// Validate nonnegativity by representation and exact simplex normalization.
    pub fn validate(&self) -> Result<()> {
        if self.outcome_count < 2 || usize::from(self.outcome_count) > MAX_OUTCOMES {
            return Err(Error::InvalidBasis);
        }
        if self.denominator == 0 {
            return Err(Error::ZeroValue);
        }
        let mut sum = 0u128;
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            if i < usize::from(self.outcome_count) {
                sum = sum
                    .checked_add(self.numerators[i])
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if self.numerators[i] != 0 {
                return Err(Error::InvalidBasis);
            }
            i += 1;
        }
        if sum != self.denominator {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }
}

/// Lifecycle phase of a bounded facility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FacilityPhase {
    /// Two-sided inventory transitions are admitted inside the trading window.
    Trading = 0,
    /// Only componentwise buyback transitions are admitted before maturity.
    BuybackOnly = 1,
    /// The facility redeemed its retained Eggs against authenticated resolution.
    Resolved = 2,
    /// Sponsor-owned terminal cash was withdrawn and the live facility may retire.
    Retired = 3,
}

/// Net Egg movement requested from one atomic call-auction settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FacilityTradeV1 {
    /// Eggs transferred from the facility to traders.
    pub sell_to_users: [u64; MAX_OUTCOMES],
    /// Eggs transferred from traders back to the facility.
    pub buy_from_users: [u64; MAX_OUTCOMES],
}

impl FacilityTradeV1 {
    /// Canonical empty flow.
    pub const EMPTY: Self = Self {
        sell_to_users: [0; MAX_OUTCOMES],
        buy_from_users: [0; MAX_OUTCOMES],
    };
}

/// Exact asset recipe for one proposed endpoint transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FacilityTradeReceiptV1 {
    /// Bound policy identity.
    pub policy_id: Id,
    /// Generation consumed by the transition.
    pub pre_generation: u64,
    /// Generation produced by the transition.
    pub post_generation: u64,
    /// Exact Egg flow.
    pub trade: FacilityTradeV1,
    /// Old facility-attributed external Egg inventory.
    pub old_inventory: [u64; MAX_OUTCOMES],
    /// New facility-attributed external Egg inventory.
    pub new_inventory: [u64; MAX_OUTCOMES],
    /// Collateral atoms paid by traders to the facility.
    pub trader_cash_in_atoms: u64,
    /// Collateral atoms paid by the facility to traders.
    pub trader_cash_out_atoms: u64,
    /// Complete sets split by moving this many collateral atoms into Hoard.
    pub split_complete_sets: u64,
    /// Complete sets merged by moving this many collateral atoms out of Hoard.
    pub merge_complete_sets: u64,
    /// Post-transition free facility cash, excluding Hoard principal.
    pub new_cash_atoms: u64,
    /// Post-transition retained Egg vector.
    pub new_retained_eggs: [u64; MAX_OUTCOMES],
}

/// Exact state of one sponsor-capitalized facility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FacilityStateV1 {
    /// Immutable facility policy.
    pub policy: FacilityPolicyV1,
    /// Canonical facility identity.
    pub facility_id: Id,
    /// Immutable sponsor/refund owner.
    pub sponsor: Id,
    /// Sponsor capital deposited before the first trade.
    pub sponsor_capital_atoms: u64,
    /// Free facility collateral, excluding Market Hoard principal.
    pub cash_atoms: u64,
    /// Facility-attributed external Egg inventory.
    pub inventory: [u64; MAX_OUTCOMES],
    /// Facility-held complement Eggs.
    pub retained_eggs: [u64; MAX_OUTCOMES],
    /// Facility-attributed Market Hoard backing: live sets or resolved claims.
    pub hoard_backing_atoms: u64,
    /// External inventory payout recorded at resolution.
    pub terminal_external_payout_atoms: u64,
    /// Terminal collateral withdrawn to the immutable sponsor.
    pub sponsor_withdrawn_atoms: u64,
    /// Monotone transition generation.
    pub generation: u64,
    /// Current lifecycle phase.
    pub phase: FacilityPhase,
}

impl FacilityStateV1 {
    /// Initialize an empty, fully capitalized facility.
    pub fn initialize(
        policy: FacilityPolicyV1,
        facility_id: Id,
        sponsor: Id,
        sponsor_capital_atoms: u64,
    ) -> Result<Self> {
        policy.validate()?;
        check_id(facility_id)?;
        check_id(sponsor)?;
        if facility_id == sponsor {
            return Err(Error::InvalidIdentity);
        }
        if sponsor_capital_atoms < policy.minimum_sponsor_capital()? {
            return Err(Error::InsufficientCapital);
        }
        if sponsor_capital_atoms > MAX_ATOMS {
            return Err(Error::ParameterOutOfRange);
        }
        let value = Self {
            policy,
            facility_id,
            sponsor,
            sponsor_capital_atoms,
            cash_atoms: sponsor_capital_atoms,
            inventory: [0; MAX_OUTCOMES],
            retained_eggs: [0; MAX_OUTCOMES],
            hoard_backing_atoms: 0,
            terminal_external_payout_atoms: 0,
            sponsor_withdrawn_atoms: 0,
            generation: 0,
            phase: FacilityPhase::Trading,
        };
        value.validate()?;
        Ok(value)
    }

    /// Recompute all backing, potential, phase, and conservation identities.
    pub fn validate(&self) -> Result<()> {
        self.policy.validate()?;
        check_id(self.facility_id)?;
        check_id(self.sponsor)?;
        if self.facility_id == self.sponsor
            || self.sponsor_capital_atoms < self.policy.minimum_sponsor_capital()?
            || self.sponsor_capital_atoms > MAX_ATOMS
        {
            return Err(Error::InvariantViolation);
        }
        validate_inventory(&self.policy, &self.inventory)?;
        let potential = rounded_quadratic_potential(&self.policy, &self.inventory)?;
        let liability = full_simplex_liability(self.policy.outcome_count, &self.inventory)?;

        match self.phase {
            FacilityPhase::Trading | FacilityPhase::BuybackOnly => {
                if self.terminal_external_payout_atoms != 0 || self.sponsor_withdrawn_atoms != 0 {
                    return Err(Error::InvariantViolation);
                }
                if self.hoard_backing_atoms != liability {
                    return Err(Error::InvariantViolation);
                }
                let expected_retained =
                    complement(self.policy.outcome_count, liability, &self.inventory)?;
                if self.retained_eggs != expected_retained {
                    return Err(Error::InvariantViolation);
                }
                let expected_cash = self
                    .sponsor_capital_atoms
                    .checked_add(potential)
                    .and_then(|value| value.checked_sub(liability))
                    .ok_or(Error::InsufficientCash)?;
                if self.cash_atoms != expected_cash {
                    return Err(Error::InvariantViolation);
                }
            }
            FacilityPhase::Resolved => {
                if self.hoard_backing_atoms != self.terminal_external_payout_atoms
                    || any_nonzero(self.policy.outcome_count, &self.retained_eggs)
                    || self.sponsor_withdrawn_atoms != 0
                {
                    return Err(Error::InvariantViolation);
                }
                let total = self
                    .cash_atoms
                    .checked_add(self.terminal_external_payout_atoms)
                    .ok_or(Error::ArithmeticOverflow)?;
                let expected = self
                    .sponsor_capital_atoms
                    .checked_add(potential)
                    .ok_or(Error::ArithmeticOverflow)?;
                if total != expected {
                    return Err(Error::InvariantViolation);
                }
            }
            FacilityPhase::Retired => {
                if self.cash_atoms != 0
                    || self.hoard_backing_atoms != self.terminal_external_payout_atoms
                    || any_nonzero(self.policy.outcome_count, &self.retained_eggs)
                {
                    return Err(Error::InvariantViolation);
                }
                let total = self
                    .sponsor_withdrawn_atoms
                    .checked_add(self.terminal_external_payout_atoms)
                    .ok_or(Error::ArithmeticOverflow)?;
                let expected = self
                    .sponsor_capital_atoms
                    .checked_add(potential)
                    .ok_or(Error::ArithmeticOverflow)?;
                if total != expected {
                    return Err(Error::InvariantViolation);
                }
            }
        }
        Ok(())
    }

    /// Exact current integer potential `ceil(C(q))` in collateral atoms.
    pub fn rounded_potential(&self) -> Result<u64> {
        rounded_quadratic_potential(&self.policy, &self.inventory)
    }

    /// Exact full-simplex liability `max_i q_i` in collateral atoms.
    pub fn liability(&self) -> Result<u64> {
        full_simplex_liability(self.policy.outcome_count, &self.inventory)
    }

    /// Exact rational instantaneous price vector before integer trade rounding.
    pub fn price_vector(&self) -> Result<PriceVectorV1> {
        quadratic_price_vector(&self.policy, &self.inventory)
    }

    /// Quote an atomic endpoint transition without mutating state.
    pub fn quote_trade(&self, slot: u64, trade: FacilityTradeV1) -> Result<FacilityTradeReceiptV1> {
        self.validate()?;
        match self.phase {
            FacilityPhase::Trading => {
                if slot < self.policy.trading_open_slot || slot >= self.policy.trading_close_slot {
                    return Err(Error::InvalidSchedule);
                }
            }
            FacilityPhase::BuybackOnly => {
                if slot >= self.policy.maturity_slot {
                    return Err(Error::InvalidSchedule);
                }
                if any_nonzero(self.policy.outcome_count, &trade.sell_to_users) {
                    return Err(Error::InvalidPhase);
                }
            }
            FacilityPhase::Resolved | FacilityPhase::Retired => {
                return Err(Error::InvalidPhase);
            }
        }
        validate_trade(&self.policy, &trade)?;

        let new_inventory = apply_trade(self.policy.outcome_count, &self.inventory, &trade)?;
        validate_inventory(&self.policy, &new_inventory)?;

        let old_potential = self.rounded_potential()?;
        let new_potential = rounded_quadratic_potential(&self.policy, &new_inventory)?;
        let (cash_in, cash_out) = if new_potential >= old_potential {
            (new_potential - old_potential, 0)
        } else {
            (0, old_potential - new_potential)
        };
        let old_liability = self.liability()?;
        let new_liability = full_simplex_liability(self.policy.outcome_count, &new_inventory)?;
        let (split, merge) = if new_liability >= old_liability {
            (new_liability - old_liability, 0)
        } else {
            (0, old_liability - new_liability)
        };

        let staged_cash = self
            .cash_atoms
            .checked_add(cash_in)
            .and_then(|value| value.checked_add(merge))
            .ok_or(Error::ArithmeticOverflow)?;
        let new_cash = staged_cash
            .checked_sub(split)
            .and_then(|value| value.checked_sub(cash_out))
            .ok_or(Error::InsufficientCash)?;
        let expected_cash = self
            .sponsor_capital_atoms
            .checked_add(new_potential)
            .and_then(|value| value.checked_sub(new_liability))
            .ok_or(Error::InsufficientCash)?;
        if new_cash != expected_cash {
            return Err(Error::InvariantViolation);
        }
        let new_retained = complement(self.policy.outcome_count, new_liability, &new_inventory)?;
        validate_egg_flow_conservation(
            self.policy.outcome_count,
            &self.retained_eggs,
            &new_retained,
            &trade,
            split,
            merge,
        )?;

        Ok(FacilityTradeReceiptV1 {
            policy_id: self.policy.policy_id,
            pre_generation: self.generation,
            post_generation: self
                .generation
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            trade,
            old_inventory: self.inventory,
            new_inventory,
            trader_cash_in_atoms: cash_in,
            trader_cash_out_atoms: cash_out,
            split_complete_sets: split,
            merge_complete_sets: merge,
            new_cash_atoms: new_cash,
            new_retained_eggs: new_retained,
        })
    }

    /// Execute an atomic endpoint transition after recomputing its exact quote.
    pub fn execute_trade(
        &mut self,
        slot: u64,
        trade: FacilityTradeV1,
    ) -> Result<FacilityTradeReceiptV1> {
        let receipt = self.quote_trade(slot, trade)?;
        let mut next = *self;
        next.inventory = receipt.new_inventory;
        next.cash_atoms = receipt.new_cash_atoms;
        next.retained_eggs = receipt.new_retained_eggs;
        next.hoard_backing_atoms =
            full_simplex_liability(next.policy.outcome_count, &next.inventory)?;
        next.generation = receipt.post_generation;
        next.validate()?;
        *self = next;
        Ok(receipt)
    }

    /// Enter buyback-only mode under the immutable sponsor's authority.
    pub fn halt_by_sponsor(&mut self, sponsor: Id) -> Result<()> {
        self.validate()?;
        if sponsor != self.sponsor {
            return Err(Error::MismatchedSponsor);
        }
        if self.phase != FacilityPhase::Trading {
            return Err(Error::InvalidPhase);
        }
        let mut next = *self;
        next.phase = FacilityPhase::BuybackOnly;
        next.generation = next
            .generation
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Permissionlessly close ordinary trading at or after the frozen close slot.
    pub fn close_trading(&mut self, slot: u64) -> Result<()> {
        self.validate()?;
        if self.phase != FacilityPhase::Trading {
            return Err(Error::InvalidPhase);
        }
        if slot < self.policy.trading_close_slot {
            return Err(Error::InvalidSchedule);
        }
        let mut next = *self;
        next.phase = FacilityPhase::BuybackOnly;
        next.generation = next
            .generation
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Resolve retained Eggs against one authenticated exact payout vector.
    ///
    /// The policy's conservative universal lot is the full payout denominator,
    /// so every facility inventory and retained-Egg coordinate divides exactly
    /// under every integer payout vector admitted by this model.
    pub fn resolve(&mut self, slot: u64, payout_weights: [u64; MAX_OUTCOMES]) -> Result<u64> {
        self.validate()?;
        if self.phase == FacilityPhase::Resolved || self.phase == FacilityPhase::Retired {
            return Err(Error::InvalidPhase);
        }
        if slot < self.policy.maturity_slot {
            return Err(Error::InvalidSchedule);
        }
        validate_payout(&self.policy, &payout_weights)?;
        let external_payout = exact_payout(
            self.policy.outcome_count,
            &self.inventory,
            &payout_weights,
            self.policy.payout_denominator,
        )?;
        let retained_payout = exact_payout(
            self.policy.outcome_count,
            &self.retained_eggs,
            &payout_weights,
            self.policy.payout_denominator,
        )?;
        let mut next = *self;
        next.cash_atoms = next
            .cash_atoms
            .checked_add(retained_payout)
            .ok_or(Error::ArithmeticOverflow)?;
        next.hoard_backing_atoms = external_payout;
        next.retained_eggs = [0; MAX_OUTCOMES];
        next.terminal_external_payout_atoms = external_payout;
        next.phase = FacilityPhase::Resolved;
        next.generation = next
            .generation
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate()?;
        let terminal_cash = next.cash_atoms;
        *self = next;
        Ok(terminal_cash)
    }

    /// Withdraw sponsor-owned terminal cash and retire a flat or resolved facility.
    pub fn withdraw_and_retire(&mut self, sponsor: Id) -> Result<u64> {
        self.validate()?;
        if sponsor != self.sponsor {
            return Err(Error::MismatchedSponsor);
        }
        let flat_unwind = self.phase == FacilityPhase::BuybackOnly
            && !any_nonzero(self.policy.outcome_count, &self.inventory);
        if self.phase != FacilityPhase::Resolved && !flat_unwind {
            return Err(Error::InvalidPhase);
        }
        let amount = self.cash_atoms;
        let mut next = *self;
        next.cash_atoms = 0;
        next.sponsor_withdrawn_atoms = amount;
        next.phase = FacilityPhase::Retired;
        next.generation = next
            .generation
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate()?;
        *self = next;
        Ok(amount)
    }

    /// Exact sponsor terminal equity at a hypothetical payout without mutation.
    pub fn terminal_equity(&self, payout_weights: &[u64; MAX_OUTCOMES]) -> Result<u64> {
        self.validate()?;
        if self.phase == FacilityPhase::Resolved || self.phase == FacilityPhase::Retired {
            return Err(Error::InvalidPhase);
        }
        validate_payout(&self.policy, payout_weights)?;
        let external = exact_payout(
            self.policy.outcome_count,
            &self.inventory,
            payout_weights,
            self.policy.payout_denominator,
        )?;
        self.sponsor_capital_atoms
            .checked_add(self.rounded_potential()?)
            .and_then(|value| value.checked_sub(external))
            .ok_or(Error::InsufficientCash)
    }
}

/// Exact full-simplex liability `max_i q_i` for nonnegative inventory.
pub fn full_simplex_liability(outcome_count: u8, inventory: &[u64; MAX_OUTCOMES]) -> Result<u64> {
    if outcome_count < 2 || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(Error::InvalidBasis);
    }
    validate_padding(outcome_count, inventory)?;
    let mut result = 0u64;
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        if inventory[i] > result {
            result = inventory[i];
        }
        i += 1;
    }
    Ok(result)
}

/// Canonical integer potential `ceil(C(q))`.
///
/// The exact rational potential is
///
/// ```text
/// C(q) = dot(pi,q) + (n*sum(q_i^2) - Q^2)/(2*b*n),
/// pi_i = initial_price_weight_i / initial_price_denominator,
/// Q = sum(q_i).
/// ```
///
/// The one ceiling creates an integer endpoint potential. Trade charges are
/// differences of this potential, so arbitrary splitting and path replay
/// telescope exactly.
pub fn rounded_quadratic_potential(
    policy: &FacilityPolicyV1,
    inventory: &[u64; MAX_OUTCOMES],
) -> Result<u64> {
    policy.validate()?;
    validate_inventory(policy, inventory)?;
    let n = u128::from(policy.outcome_count);
    let b = u128::from(policy.depth_atoms);
    let price_denominator = u128::from(policy.initial_price_denominator);
    let (sum, sum_squares) = moments(policy.outcome_count, inventory)?;
    let initial_dot = weighted_sum(
        policy.outcome_count,
        inventory,
        &policy.initial_price_weights,
    )?;
    let variance_left = n
        .checked_mul(sum_squares)
        .ok_or(Error::ArithmeticOverflow)?;
    let variance_right = sum.checked_mul(sum).ok_or(Error::ArithmeticOverflow)?;
    let variance_numerator = variance_left
        .checked_sub(variance_right)
        .ok_or(Error::InvariantViolation)?;
    let linear_numerator = b
        .checked_mul(2)
        .and_then(|value| value.checked_mul(n))
        .and_then(|value| value.checked_mul(initial_dot))
        .ok_or(Error::ArithmeticOverflow)?;
    let quadratic_numerator = variance_numerator
        .checked_mul(price_denominator)
        .ok_or(Error::ArithmeticOverflow)?;
    let numerator = linear_numerator
        .checked_add(quadratic_numerator)
        .ok_or(Error::ArithmeticOverflow)?;
    let denominator = b
        .checked_mul(2)
        .and_then(|value| value.checked_mul(n))
        .and_then(|value| value.checked_mul(price_denominator))
        .ok_or(Error::ArithmeticOverflow)?;
    let result = ceil_div(numerator, denominator)?;
    let value = u64::try_from(result).map_err(|_| Error::ArithmeticOverflow)?;
    let liability = full_simplex_liability(policy.outcome_count, inventory)?;
    if value > liability {
        return Err(Error::InvariantViolation);
    }
    Ok(value)
}

/// Exact rational gradient prices of the unrounded quadratic potential.
pub fn quadratic_price_vector(
    policy: &FacilityPolicyV1,
    inventory: &[u64; MAX_OUTCOMES],
) -> Result<PriceVectorV1> {
    policy.validate()?;
    validate_inventory(policy, inventory)?;
    let n = u128::from(policy.outcome_count);
    let b = u128::from(policy.depth_atoms);
    let price_denominator = u128::from(policy.initial_price_denominator);
    let (sum, _) = moments(policy.outcome_count, inventory)?;
    let denominator = b
        .checked_mul(n)
        .and_then(|value| value.checked_mul(price_denominator))
        .ok_or(Error::ArithmeticOverflow)?;
    let mut numerators = [0u128; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        let initial = u128::from(policy.initial_price_weights[i])
            .checked_mul(b)
            .and_then(|value| value.checked_mul(n))
            .ok_or(Error::ArithmeticOverflow)?;
        let displacement = price_denominator
            .checked_mul(n)
            .and_then(|value| value.checked_mul(u128::from(inventory[i])))
            .ok_or(Error::ArithmeticOverflow)?;
        let offset = price_denominator
            .checked_mul(sum)
            .ok_or(Error::ArithmeticOverflow)?;
        let value = initial
            .checked_add(displacement)
            .ok_or(Error::ArithmeticOverflow)?
            .checked_sub(offset)
            .ok_or(Error::PriceDomain)?;
        numerators[i] = value;
        i += 1;
    }
    let result = PriceVectorV1 {
        numerators,
        denominator,
        outcome_count: policy.outcome_count,
    };
    result.validate()?;
    Ok(result)
}

fn validate_inventory(policy: &FacilityPolicyV1, inventory: &[u64; MAX_OUTCOMES]) -> Result<()> {
    validate_padding(policy.outcome_count, inventory)?;
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        if inventory[i] > policy.max_inventory[i] {
            return Err(Error::InventoryLimit);
        }
        if !inventory[i].is_multiple_of(policy.payout_denominator) {
            return Err(Error::NonIntegralLot);
        }
        i += 1;
    }
    let (sum, _) = moments(policy.outcome_count, inventory)?;
    let n = u128::from(policy.outcome_count);
    let b = u128::from(policy.depth_atoms);
    let price_denominator = u128::from(policy.initial_price_denominator);
    i = 0;
    while i < usize::from(policy.outcome_count) {
        let positive = u128::from(policy.initial_price_weights[i])
            .checked_mul(b)
            .and_then(|value| value.checked_mul(n))
            .and_then(|value| {
                price_denominator
                    .checked_mul(n)
                    .and_then(|scale| scale.checked_mul(u128::from(inventory[i])))
                    .and_then(|displacement| value.checked_add(displacement))
            })
            .ok_or(Error::ArithmeticOverflow)?;
        let negative = price_denominator
            .checked_mul(sum)
            .ok_or(Error::ArithmeticOverflow)?;
        if positive < negative {
            return Err(Error::PriceDomain);
        }
        i += 1;
    }
    Ok(())
}

fn validate_trade(policy: &FacilityPolicyV1, trade: &FacilityTradeV1) -> Result<()> {
    validate_padding(policy.outcome_count, &trade.sell_to_users)?;
    validate_padding(policy.outcome_count, &trade.buy_from_users)?;
    let mut any = false;
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        let sell = trade.sell_to_users[i];
        let buy = trade.buy_from_users[i];
        if sell != 0 && buy != 0 {
            return Err(Error::NonCanonicalFlow);
        }
        if !sell.is_multiple_of(policy.payout_denominator)
            || !buy.is_multiple_of(policy.payout_denominator)
        {
            return Err(Error::NonIntegralLot);
        }
        any |= sell != 0 || buy != 0;
        i += 1;
    }
    if !any {
        return Err(Error::ZeroValue);
    }
    Ok(())
}

fn apply_trade(
    outcome_count: u8,
    inventory: &[u64; MAX_OUTCOMES],
    trade: &FacilityTradeV1,
) -> Result<[u64; MAX_OUTCOMES]> {
    let mut result = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        result[i] = inventory[i]
            .checked_add(trade.sell_to_users[i])
            .ok_or(Error::ArithmeticOverflow)?
            .checked_sub(trade.buy_from_users[i])
            .ok_or(Error::InsufficientInventory)?;
        i += 1;
    }
    Ok(result)
}

fn validate_egg_flow_conservation(
    outcome_count: u8,
    old_retained: &[u64; MAX_OUTCOMES],
    new_retained: &[u64; MAX_OUTCOMES],
    trade: &FacilityTradeV1,
    split: u64,
    merge: u64,
) -> Result<()> {
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        let inputs = old_retained[i]
            .checked_add(trade.buy_from_users[i])
            .and_then(|value| value.checked_add(split))
            .ok_or(Error::ArithmeticOverflow)?;
        let outputs = new_retained[i]
            .checked_add(trade.sell_to_users[i])
            .and_then(|value| value.checked_add(merge))
            .ok_or(Error::ArithmeticOverflow)?;
        if inputs != outputs {
            return Err(Error::InvariantViolation);
        }
        i += 1;
    }
    Ok(())
}

fn complement(
    outcome_count: u8,
    liability: u64,
    inventory: &[u64; MAX_OUTCOMES],
) -> Result<[u64; MAX_OUTCOMES]> {
    let mut result = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        result[i] = liability
            .checked_sub(inventory[i])
            .ok_or(Error::InvariantViolation)?;
        i += 1;
    }
    Ok(result)
}

fn validate_payout(policy: &FacilityPolicyV1, weights: &[u64; MAX_OUTCOMES]) -> Result<()> {
    validate_padding(policy.outcome_count, weights)?;
    let mut sum = 0u128;
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        sum = sum
            .checked_add(u128::from(weights[i]))
            .ok_or(Error::ArithmeticOverflow)?;
        i += 1;
    }
    if sum != u128::from(policy.payout_denominator) {
        return Err(Error::InvalidPayoutVector);
    }
    Ok(())
}

fn exact_payout(
    outcome_count: u8,
    inventory: &[u64; MAX_OUTCOMES],
    weights: &[u64; MAX_OUTCOMES],
    denominator: u64,
) -> Result<u64> {
    let mut numerator = 0u128;
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        numerator = numerator
            .checked_add(
                u128::from(inventory[i])
                    .checked_mul(u128::from(weights[i]))
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        i += 1;
    }
    let denominator = u128::from(denominator);
    if !numerator.is_multiple_of(denominator) {
        return Err(Error::InvariantViolation);
    }
    u64::try_from(numerator / denominator).map_err(|_| Error::ArithmeticOverflow)
}

fn moments(outcome_count: u8, inventory: &[u64; MAX_OUTCOMES]) -> Result<(u128, u128)> {
    let mut sum = 0u128;
    let mut sum_squares = 0u128;
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        let value = u128::from(inventory[i]);
        sum = sum.checked_add(value).ok_or(Error::ArithmeticOverflow)?;
        sum_squares = sum_squares
            .checked_add(value.checked_mul(value).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)?;
        i += 1;
    }
    Ok((sum, sum_squares))
}

fn any_nonzero(outcome_count: u8, values: &[u64; MAX_OUTCOMES]) -> bool {
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        if values[i] != 0 {
            return true;
        }
        i += 1;
    }
    false
}

fn validate_initial_price(policy: &FacilityPolicyV1) -> Result<()> {
    validate_padding(policy.outcome_count, &policy.initial_price_weights)?;
    let mut sum = 0u128;
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        sum = sum
            .checked_add(u128::from(policy.initial_price_weights[i]))
            .ok_or(Error::ArithmeticOverflow)?;
        i += 1;
    }
    if sum != u128::from(policy.initial_price_denominator) {
        return Err(Error::InvalidPriceVector);
    }
    Ok(())
}

fn weighted_sum(
    outcome_count: u8,
    left: &[u64; MAX_OUTCOMES],
    right: &[u64; MAX_OUTCOMES],
) -> Result<u128> {
    let mut result = 0u128;
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        result = result
            .checked_add(
                u128::from(left[i])
                    .checked_mul(u128::from(right[i]))
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        i += 1;
    }
    Ok(result)
}

fn validate_padding(outcome_count: u8, values: &[u64; MAX_OUTCOMES]) -> Result<()> {
    if outcome_count < 2 || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(Error::InvalidBasis);
    }
    let mut i = usize::from(outcome_count);
    while i < MAX_OUTCOMES {
        if values[i] != 0 {
            return Err(Error::InvalidBasis);
        }
        i += 1;
    }
    Ok(())
}

fn ceil_div(numerator: u128, denominator: u128) -> Result<u128> {
    if denominator == 0 {
        return Err(Error::ZeroValue);
    }
    if numerator == 0 {
        return Ok(0);
    }
    numerator
        .checked_sub(1)
        .and_then(|value| value.checked_div(denominator))
        .and_then(|value| value.checked_add(1))
        .ok_or(Error::ArithmeticOverflow)
}

fn check_id(id: Id) -> Result<()> {
    if id == [0; 32] {
        return Err(Error::InvalidIdentity);
    }
    Ok(())
}
