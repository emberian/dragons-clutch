//! Transactional custody, supply, unwind, donation, and redemption algebra.

use core::convert::TryFrom;

use clutch_kernel::{BasisMode, Error as BaseError, MarketState, Phase, Position};

use crate::{
    gcd_u128, Amount, BackingPlan, Error, NativeBasisIdentity, NativeClaim, Result, MAX_OUTCOMES,
};

/// Active or terminal lifecycle of the authenticated base Market.
pub use clutch_kernel::Phase as MarketPhase;

/// Canonically padded exact resolved simplex weights.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct ResolvedWeights {
    /// Common denominator shared with immutable Terms.
    pub denominator: u64,
    /// Active nonnegative weights summing exactly to `denominator`.
    pub weights: [u64; MAX_OUTCOMES],
}

impl ResolvedWeights {
    /// Empty sentinel used while a Market is Active.
    pub const ZERO: Self = Self {
        denominator: 0,
        weights: [0; MAX_OUTCOMES],
    };

    /// Validate this vector against one immutable native basis.
    pub fn validate(&self, basis: &NativeBasisIdentity) -> Result<()> {
        basis.validate()?;
        if self.denominator != basis.denominator {
            return Err(Error::InvalidDenominator);
        }
        let count = usize::from(basis.outcome_count);
        let mut sum = 0_u64;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let weight = self.weights[index];
            if index < count {
                if weight > self.denominator {
                    return Err(Error::InvalidWeights);
                }
                sum = sum.checked_add(weight).ok_or(Error::ArithmeticOverflow)?;
            } else if weight != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if sum != self.denominator {
            return Err(Error::InvalidWeights);
        }
        Ok(())
    }
}

/// Immutable identity joined to the complete base semantic state.
///
/// This is not a second persisted Market truth. The adapter reconstructs it
/// from authenticated base state before each call. The embedded base kernel
/// remains the owner of global collateral sufficiency and applies every exact
/// supply/Hoard delta caused by wrapper routes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct MarketLedger {
    /// Immutable native basis identity.
    pub basis: NativeBasisIdentity,
    /// Existing base kernel state: the sole owner of total supply, Hoard
    /// collateral, payout-set semantics, and their global invariant.
    pub base: MarketState,
}

impl MarketLedger {
    /// Bind an immutable identity to an already validated base kernel state.
    pub fn from_base(basis: NativeBasisIdentity, base: MarketState) -> Result<Self> {
        let value = Self { basis, base };
        value.validate()?;
        Ok(value)
    }

    /// Validate the identity join and the base kernel's complete invariant.
    pub fn validate(&self) -> Result<()> {
        self.basis.validate()?;
        self.base
            .check_invariants()
            .map_err(|_| Error::InvariantViolation)?;
        if self.base.outcomes != self.basis.outcome_count
            || self.base.payouts.vectors[0].denominator != self.basis.denominator
        {
            return Err(Error::DifferentBasis);
        }
        Ok(())
    }

    /// Current base phase.
    pub const fn phase(&self) -> MarketPhase {
        self.base.phase
    }

    /// Current Hoard collateral atoms.
    pub const fn hoard_atoms(&self) -> Amount {
        self.base.collateral
    }

    /// Current total claim supply, internal plus external.
    pub const fn total_supply(&self) -> &[Amount; MAX_OUTCOMES] {
        &self.base.total_supply
    }

    /// Return the one effective resolved payout vector.
    pub fn resolved_weights(&self) -> Result<ResolvedWeights> {
        self.validate()?;
        if self.base.phase != Phase::Resolved {
            return Err(Error::NotResolved);
        }
        let vector = match self.base.basis_mode {
            BasisMode::FinitePreset => self
                .base
                .payouts
                .vectors
                .get(usize::from(self.base.resolved_payout))
                .ok_or(Error::InvariantViolation)?,
            BasisMode::DerivedBasis => &self.base.resolved_vector,
        };
        let value = ResolvedWeights {
            denominator: vector.denominator,
            weights: vector.weights,
        };
        value.validate(&self.basis)?;
        Ok(value)
    }
}

/// Free assets in the wrapper's base Position vault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct BackingVault {
    /// Free Position cash atoms.
    pub cash_atoms: Amount,
    /// Free internal native Eggs.
    pub internal: [Amount; MAX_OUTCOMES],
}

impl BackingVault {
    /// Empty canonical vault.
    pub const EMPTY: Self = Self {
        cash_atoms: 0,
        internal: [0; MAX_OUTCOMES],
    };

    fn validate_padding(&self, outcome_count: u8) -> Result<()> {
        let mut index = usize::from(outcome_count);
        while index < MAX_OUTCOMES {
            if self.internal[index] != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        Ok(())
    }
}

/// Free base Position assets and wrapper-token balance participating in a route.
///
/// The adapter may bind the Position and token account to different explicit
/// beneficiaries; this core intentionally carries no owner keys. It must still
/// authenticate each account, authority, and nonaliasing rule before applying
/// the staged deltas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct HolderAssets {
    /// Free Position cash, excluding authenticated reservations.
    pub cash_atoms: Amount,
    /// Free internal native Eggs, excluding seller reservations.
    pub internal: [Amount; MAX_OUTCOMES],
    /// Authenticated extension-free Token-2022 wrapper balance.
    pub wrapper_atoms: Amount,
}

impl HolderAssets {
    /// Empty canonical holder projection.
    pub const EMPTY: Self = Self {
        cash_atoms: 0,
        internal: [0; MAX_OUTCOMES],
        wrapper_atoms: 0,
    };

    fn validate(&self, outcome_count: u8, actual_supply: Amount) -> Result<()> {
        let mut index = usize::from(outcome_count);
        while index < MAX_OUTCOMES {
            if self.internal[index] != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if self.wrapper_atoms > actual_supply {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }
}

/// Token supply and permanent retirement state authenticated by the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct WrapperState {
    /// Actual Token-2022 mint supply; never a descriptor-maintained shadow.
    pub actual_supply: Amount,
    /// Permanent descriptor tombstone flag.
    pub retired: bool,
}

/// Assets donated during exact surplus compaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DonationDelta {
    /// Surplus cash transferred to the base Hoard.
    pub cash_to_hoard: Amount,
    /// Surplus internal Eggs destroyed without collateral release.
    pub eggs_destroyed: [Amount; MAX_OUTCOMES],
}

/// Complete structured-claim state over one native claim.
///
/// Every mutating method validates the input, stages all checked arithmetic in
/// copies, validates the prospective state, then commits every field. Therefore
/// every refusal leaves all arguments byte-for-byte unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct StructuredClaimMachine {
    /// Native payoff and identity semantics.
    pub claim: NativeClaim,
    /// Derived complete-set-compressed custody policy.
    pub backing: BackingPlan,
    /// Authenticated wrapper supply and lifecycle.
    pub wrapper: WrapperState,
    /// Wrapper-controlled base Position assets.
    pub vault: BackingVault,
}

impl StructuredClaimMachine {
    /// Construct an empty active-capable descriptor model.
    pub fn new(claim: NativeClaim) -> Result<Self> {
        claim.validate()?;
        let value = Self {
            backing: claim.vector.backing_plan()?,
            claim,
            wrapper: WrapperState {
                actual_supply: 0,
                retired: false,
            },
            vault: BackingVault::EMPTY,
        };
        Ok(value)
    }

    /// Restore authenticated persisted/token state and check exact coverage.
    pub fn restore(
        claim: NativeClaim,
        wrapper: WrapperState,
        vault: BackingVault,
        market: &MarketLedger,
    ) -> Result<Self> {
        claim.validate()?;
        let value = Self {
            backing: claim.vector.backing_plan()?,
            claim,
            wrapper,
            vault,
        };
        value.check_invariants(market)?;
        Ok(value)
    }

    /// Check basis binding, canonical vault padding, and exact supply coverage.
    ///
    /// Donations may make backing greater than the required vector. A deficit
    /// in any component refuses every route.
    pub fn check_invariants(&self, market: &MarketLedger) -> Result<()> {
        self.claim.validate()?;
        market.validate()?;
        if self.claim.basis != market.basis
            || self.backing.outcome_count != market.basis.outcome_count
        {
            return Err(Error::DifferentBasis);
        }
        if self.backing != self.claim.vector.backing_plan()? {
            return Err(Error::InvariantViolation);
        }
        self.vault
            .validate_padding(self.claim.basis.outcome_count)?;
        let required_cash = self
            .wrapper
            .actual_supply
            .checked_mul(self.backing.cash_per_wrapper)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.vault.cash_atoms < required_cash {
            return Err(Error::UnderCollateralized);
        }
        let count = usize::from(self.claim.basis.outcome_count);
        let mut index = 0_usize;
        while index < count {
            let required = self
                .wrapper
                .actual_supply
                .checked_mul(self.backing.residual_eggs_per_wrapper[index])
                .ok_or(Error::ArithmeticOverflow)?;
            if self.vault.internal[index] < required {
                return Err(Error::UnderCollateralized);
            }
            if market.base.total_supply[index] < self.vault.internal[index] {
                return Err(Error::InvariantViolation);
            }
            index += 1;
        }
        if self.wrapper.retired
            && (self.wrapper.actual_supply != 0
                || self.vault.cash_atoms != 0
                || self.vault.internal != [0; MAX_OUTCOMES])
        {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }

    /// Mint wrappers from already-compressed free cash and residual Eggs.
    ///
    /// This ownership-preserving custody move is valid while Active or
    /// Resolved. Resolution fixes value but does not make already-owned cash
    /// and Eggs ineligible for an exactly backed bearer representation.
    pub fn wrap_canonical(
        &mut self,
        market: &MarketLedger,
        holder: &mut HolderAssets,
        quantity: Amount,
    ) -> Result<()> {
        self.preflight(market, holder, quantity, false)?;
        let cash = quantity
            .checked_mul(self.backing.cash_per_wrapper)
            .ok_or(Error::ArithmeticOverflow)?;
        if holder.cash_atoms < cash {
            return Err(Error::InsufficientCash);
        }
        let mut next = *self;
        let mut next_holder = *holder;
        next.wrapper.actual_supply = next
            .wrapper
            .actual_supply
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        next.vault.cash_atoms = next
            .vault
            .cash_atoms
            .checked_add(cash)
            .ok_or(Error::ArithmeticOverflow)?;
        next_holder.cash_atoms = next_holder
            .cash_atoms
            .checked_sub(cash)
            .ok_or(Error::ArithmeticUnderflow)?;
        next_holder.wrapper_atoms = next_holder
            .wrapper_atoms
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        let count = usize::from(self.claim.basis.outcome_count);
        let mut index = 0_usize;
        while index < count {
            let eggs = quantity
                .checked_mul(self.backing.residual_eggs_per_wrapper[index])
                .ok_or(Error::ArithmeticOverflow)?;
            if next_holder.internal[index] < eggs {
                return Err(Error::InsufficientEggs);
            }
            next_holder.internal[index] = next_holder.internal[index]
                .checked_sub(eggs)
                .ok_or(Error::ArithmeticUnderflow)?;
            next.vault.internal[index] = next.vault.internal[index]
                .checked_add(eggs)
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        next.check_invariants(market)?;
        next.validate_holder_against_market(market, &next_holder)?;
        *self = next;
        *holder = next_holder;
        Ok(())
    }

    /// Mint wrappers from full native vectors, merging their complete-set floor.
    ///
    /// A positive floor requires Active base Merge. A zero-floor vector is
    /// already canonical and therefore remains available after resolution.
    pub fn wrap_full(
        &mut self,
        market: &mut MarketLedger,
        holder: &mut HolderAssets,
        quantity: Amount,
    ) -> Result<()> {
        self.preflight(market, holder, quantity, false)?;
        let mut next = *self;
        let mut next_market = *market;
        let mut next_holder = *holder;
        let merged_cash = quantity
            .checked_mul(self.backing.cash_per_wrapper)
            .ok_or(Error::ArithmeticOverflow)?;
        if merged_cash != 0 && market.base.phase != MarketPhase::Active {
            return Err(Error::NotActive);
        }
        next.wrapper.actual_supply = next
            .wrapper
            .actual_supply
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        next_holder.wrapper_atoms = next_holder
            .wrapper_atoms
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        next.vault.cash_atoms = next
            .vault
            .cash_atoms
            .checked_add(merged_cash)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut merge_position = Position::EMPTY;
        let mut expected_residual = [0_u64; MAX_OUTCOMES];
        let count = usize::from(self.claim.basis.outcome_count);
        let mut index = 0_usize;
        while index < count {
            let full = quantity
                .checked_mul(self.claim.vector.coefficients[index])
                .ok_or(Error::ArithmeticOverflow)?;
            let residual = quantity
                .checked_mul(self.backing.residual_eggs_per_wrapper[index])
                .ok_or(Error::ArithmeticOverflow)?;
            if next_holder.internal[index] < full {
                return Err(Error::InsufficientEggs);
            }
            merge_position.internal[index] = full;
            expected_residual[index] = residual;
            next_holder.internal[index] = next_holder.internal[index]
                .checked_sub(full)
                .ok_or(Error::ArithmeticUnderflow)?;
            next.vault.internal[index] = next.vault.internal[index]
                .checked_add(residual)
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        if merged_cash != 0 {
            next_market
                .base
                .merge(&mut merge_position, merged_cash)
                .map_err(map_base_error)?;
        }
        if merge_position.internal != expected_residual {
            return Err(Error::InvariantViolation);
        }
        next.check_invariants(&next_market)?;
        next.validate_holder_against_market(&next_market, &next_holder)?;
        *self = next;
        *market = next_market;
        *holder = next_holder;
        Ok(())
    }

    /// Burn wrappers and return canonical cash-plus-residual backing in any phase.
    pub fn unwind_canonical(
        &mut self,
        market: &MarketLedger,
        holder: &mut HolderAssets,
        quantity: Amount,
    ) -> Result<()> {
        self.preflight(market, holder, quantity, false)?;
        self.require_holder_wrappers(holder, quantity)?;
        let cash = quantity
            .checked_mul(self.backing.cash_per_wrapper)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut next = *self;
        let mut next_holder = *holder;
        next.wrapper.actual_supply = next
            .wrapper
            .actual_supply
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        next_holder.wrapper_atoms = next_holder
            .wrapper_atoms
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        next.vault.cash_atoms = next
            .vault
            .cash_atoms
            .checked_sub(cash)
            .ok_or(Error::UnderCollateralized)?;
        next_holder.cash_atoms = next_holder
            .cash_atoms
            .checked_add(cash)
            .ok_or(Error::ArithmeticOverflow)?;
        let count = usize::from(self.claim.basis.outcome_count);
        let mut index = 0_usize;
        while index < count {
            let eggs = quantity
                .checked_mul(self.backing.residual_eggs_per_wrapper[index])
                .ok_or(Error::ArithmeticOverflow)?;
            next.vault.internal[index] = next.vault.internal[index]
                .checked_sub(eggs)
                .ok_or(Error::UnderCollateralized)?;
            next_holder.internal[index] = next_holder.internal[index]
                .checked_add(eggs)
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        next.check_invariants(market)?;
        next.validate_holder_against_market(market, &next_holder)?;
        *self = next;
        *holder = next_holder;
        Ok(())
    }

    /// Burn wrappers, split their cash floor, and return full vectors.
    ///
    /// A positive floor requires Active base Split. A zero-floor claim already
    /// has full-vector canonical backing and may unwind in either phase.
    pub fn unwind_full(
        &mut self,
        market: &mut MarketLedger,
        holder: &mut HolderAssets,
        quantity: Amount,
    ) -> Result<()> {
        self.preflight(market, holder, quantity, false)?;
        self.require_holder_wrappers(holder, quantity)?;
        let split_cash = quantity
            .checked_mul(self.backing.cash_per_wrapper)
            .ok_or(Error::ArithmeticOverflow)?;
        if split_cash != 0 && market.base.phase != MarketPhase::Active {
            return Err(Error::NotActive);
        }
        let mut next = *self;
        let mut next_market = *market;
        let mut next_holder = *holder;
        next.wrapper.actual_supply = next
            .wrapper
            .actual_supply
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        next_holder.wrapper_atoms = next_holder
            .wrapper_atoms
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        next.vault.cash_atoms = next
            .vault
            .cash_atoms
            .checked_sub(split_cash)
            .ok_or(Error::UnderCollateralized)?;
        let mut split_position = Position::EMPTY;
        let mut expected_full = [0_u64; MAX_OUTCOMES];
        let count = usize::from(self.claim.basis.outcome_count);
        let mut index = 0_usize;
        while index < count {
            let residual = quantity
                .checked_mul(self.backing.residual_eggs_per_wrapper[index])
                .ok_or(Error::ArithmeticOverflow)?;
            let full = quantity
                .checked_mul(self.claim.vector.coefficients[index])
                .ok_or(Error::ArithmeticOverflow)?;
            split_position.internal[index] = residual;
            expected_full[index] = full;
            next.vault.internal[index] = next.vault.internal[index]
                .checked_sub(residual)
                .ok_or(Error::UnderCollateralized)?;
            next_holder.internal[index] = next_holder.internal[index]
                .checked_add(full)
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        if split_cash != 0 {
            next_market
                .base
                .split(&mut split_position, split_cash)
                .map_err(map_base_error)?;
        }
        if split_position.internal != expected_full {
            return Err(Error::InvariantViolation);
        }
        next.check_invariants(&next_market)?;
        next.validate_holder_against_market(&next_market, &next_holder)?;
        *self = next;
        *market = next_market;
        *holder = next_holder;
        Ok(())
    }

    /// Model a permissionless direct Token-2022 burn that releases no backing.
    pub fn direct_burn(
        &mut self,
        market: &MarketLedger,
        holder: &mut HolderAssets,
        quantity: Amount,
    ) -> Result<()> {
        self.preflight(market, holder, quantity, false)?;
        self.require_holder_wrappers(holder, quantity)?;
        let mut next = *self;
        let mut next_holder = *holder;
        next.wrapper.actual_supply = next
            .wrapper
            .actual_supply
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        next_holder.wrapper_atoms = next_holder
            .wrapper_atoms
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        next.check_invariants(market)?;
        next.validate_holder_against_market(market, &next_holder)?;
        *self = next;
        *holder = next_holder;
        Ok(())
    }

    /// Donate every surplus component created by direct burns, paying nobody.
    ///
    /// Cash enters the base Hoard. Residual Eggs are destroyed without releasing
    /// Hoard collateral. Partial or caller-directed surplus extraction has no
    /// representation in this API.
    pub fn compact_donation(&mut self, market: &mut MarketLedger) -> Result<DonationDelta> {
        self.check_invariants(market)?;
        self.require_live()?;
        let required_cash = self
            .wrapper
            .actual_supply
            .checked_mul(self.backing.cash_per_wrapper)
            .ok_or(Error::ArithmeticOverflow)?;
        let cash_surplus = self
            .vault
            .cash_atoms
            .checked_sub(required_cash)
            .ok_or(Error::UnderCollateralized)?;
        let mut eggs_surplus = [0_u64; MAX_OUTCOMES];
        let count = usize::from(self.claim.basis.outcome_count);
        let mut index = 0_usize;
        while index < count {
            let required = self
                .wrapper
                .actual_supply
                .checked_mul(self.backing.residual_eggs_per_wrapper[index])
                .ok_or(Error::ArithmeticOverflow)?;
            eggs_surplus[index] = self.vault.internal[index]
                .checked_sub(required)
                .ok_or(Error::UnderCollateralized)?;
            if market.base.total_supply[index] < eggs_surplus[index] {
                return Err(Error::InsufficientEggs);
            }
            index += 1;
        }
        let mut next = *self;
        let mut next_market = *market;
        next.vault.cash_atoms = required_cash;
        if cash_surplus != 0 {
            next_market
                .base
                .donate_collateral(cash_surplus)
                .map_err(map_base_error)?;
        }
        let mut donation_position = Position::EMPTY;
        let mut has_egg_surplus = false;
        index = 0;
        while index < count {
            donation_position.internal[index] = eggs_surplus[index];
            has_egg_surplus |= eggs_surplus[index] != 0;
            next.vault.internal[index] = next.vault.internal[index]
                .checked_sub(eggs_surplus[index])
                .ok_or(Error::ArithmeticUnderflow)?;
            index += 1;
        }
        if has_egg_surplus {
            next_market
                .base
                .donate_internal_vector(&mut donation_position, eggs_surplus)
                .map_err(map_base_error)?;
            if donation_position.internal != [0; MAX_OUTCOMES] {
                return Err(Error::InvariantViolation);
            }
        }
        next.check_invariants(&next_market)?;
        *self = next;
        *market = next_market;
        Ok(DonationDelta {
            cash_to_hoard: cash_surplus,
            eggs_destroyed: eggs_surplus,
        })
    }

    /// Smallest wrapper quantity whose aggregate terminal payout is integral.
    pub fn terminal_lot(&self, market: &MarketLedger) -> Result<Amount> {
        self.check_invariants(market)?;
        let resolved = market.resolved_weights()?;
        let mut numerator = 0_u128;
        let count = usize::from(self.claim.basis.outcome_count);
        let mut index = 0_usize;
        while index < count {
            let term = u128::from(self.claim.vector.coefficients[index])
                .checked_mul(u128::from(resolved.weights[index]))
                .ok_or(Error::ArithmeticOverflow)?;
            numerator = numerator
                .checked_add(term)
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        let denominator = u128::from(resolved.denominator);
        let divisor = gcd_u128(denominator, numerator);
        Amount::try_from(
            denominator
                .checked_div(divisor)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .map_err(|_| Error::ArithmeticOverflow)
    }

    /// Burn an exact terminal lot and pay aggregate vector value directly.
    ///
    /// The cash floor is transferred from the vault. Residual Eggs and their
    /// exact payout are removed from the base supply and Hoard together. No
    /// floor rounding or protocol dust recipient exists.
    pub fn redeem_terminal(
        &mut self,
        market: &mut MarketLedger,
        holder: &mut HolderAssets,
        quantity: Amount,
    ) -> Result<Amount> {
        self.preflight(market, holder, quantity, false)?;
        let resolved = market.resolved_weights()?;
        self.require_holder_wrappers(holder, quantity)?;
        let lot = self.terminal_lot(market)?;
        if !quantity.is_multiple_of(lot) {
            return Err(Error::InexactRedemption);
        }
        let cash_payout = quantity
            .checked_mul(self.backing.cash_per_wrapper)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut residual_amounts = [0_u64; MAX_OUTCOMES];
        let mut residual_numerator = 0_u128;
        let count = usize::from(self.claim.basis.outcome_count);
        let mut index = 0_usize;
        while index < count {
            residual_amounts[index] = quantity
                .checked_mul(self.backing.residual_eggs_per_wrapper[index])
                .ok_or(Error::ArithmeticOverflow)?;
            let term = u128::from(residual_amounts[index])
                .checked_mul(u128::from(resolved.weights[index]))
                .ok_or(Error::ArithmeticOverflow)?;
            residual_numerator = residual_numerator
                .checked_add(term)
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        let denominator = u128::from(resolved.denominator);
        if !residual_numerator.is_multiple_of(denominator) {
            return Err(Error::InexactRedemption);
        }
        let residual_payout = Amount::try_from(residual_numerator / denominator)
            .map_err(|_| Error::ArithmeticOverflow)?;
        let total_payout = cash_payout
            .checked_add(residual_payout)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.vault.cash_atoms < cash_payout || market.base.collateral < residual_payout {
            return Err(Error::InsufficientCash);
        }
        let mut next = *self;
        let mut next_market = *market;
        let mut next_holder = *holder;
        next.wrapper.actual_supply = next
            .wrapper
            .actual_supply
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        next_holder.wrapper_atoms = next_holder
            .wrapper_atoms
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        next.vault.cash_atoms = next
            .vault
            .cash_atoms
            .checked_sub(cash_payout)
            .ok_or(Error::UnderCollateralized)?;
        next_holder.cash_atoms = next_holder
            .cash_atoms
            .checked_add(total_payout)
            .ok_or(Error::ArithmeticOverflow)?;
        index = 0;
        let mut redemption_position = Position::EMPTY;
        while index < count {
            redemption_position.internal[index] = residual_amounts[index];
            next.vault.internal[index] = next.vault.internal[index]
                .checked_sub(residual_amounts[index])
                .ok_or(Error::UnderCollateralized)?;
            index += 1;
        }
        if residual_amounts[..count].iter().any(|amount| *amount != 0) {
            let paid = next_market
                .base
                .redeem_internal_vector_exact(&mut redemption_position, residual_amounts)
                .map_err(map_base_error)?;
            if paid != residual_payout || redemption_position.internal != [0; MAX_OUTCOMES] {
                return Err(Error::InvariantViolation);
            }
        } else if residual_payout != 0 {
            return Err(Error::InvariantViolation);
        }
        next.check_invariants(&next_market)?;
        next.validate_holder_against_market(&next_market, &next_holder)?;
        *self = next;
        *market = next_market;
        *holder = next_holder;
        Ok(total_payout)
    }

    /// Transfer wrapper ownership without changing supply or backing.
    pub fn transfer_wrappers(
        &self,
        market: &MarketLedger,
        from: &mut HolderAssets,
        to: &mut HolderAssets,
        quantity: Amount,
    ) -> Result<()> {
        self.check_invariants(market)?;
        self.require_live()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        from.validate(self.claim.basis.outcome_count, self.wrapper.actual_supply)?;
        to.validate(self.claim.basis.outcome_count, self.wrapper.actual_supply)?;
        if from.wrapper_atoms < quantity {
            return Err(Error::InsufficientWrappers);
        }
        let total_observed = from
            .wrapper_atoms
            .checked_add(to.wrapper_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        if total_observed > self.wrapper.actual_supply {
            return Err(Error::InvariantViolation);
        }
        let mut next_from = *from;
        let mut next_to = *to;
        next_from.wrapper_atoms = next_from
            .wrapper_atoms
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        next_to.wrapper_atoms = next_to
            .wrapper_atoms
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        next_from.validate(self.claim.basis.outcome_count, self.wrapper.actual_supply)?;
        next_to.validate(self.claim.basis.outcome_count, self.wrapper.actual_supply)?;
        *from = next_from;
        *to = next_to;
        Ok(())
    }

    /// Permanently retire an empty descriptor/mint identity tombstone.
    pub fn retire(&mut self, market: &MarketLedger) -> Result<()> {
        self.check_invariants(market)?;
        self.require_live()?;
        if self.wrapper.actual_supply != 0
            || self.vault.cash_atoms != 0
            || self.vault.internal != [0; MAX_OUTCOMES]
        {
            return Err(Error::RetirementBlocked);
        }
        self.wrapper.retired = true;
        self.check_invariants(market)
    }

    fn preflight(
        &self,
        market: &MarketLedger,
        holder: &HolderAssets,
        quantity: Amount,
        active_required: bool,
    ) -> Result<()> {
        self.check_invariants(market)?;
        self.require_live()?;
        self.validate_holder_against_market(market, holder)?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        if active_required && market.base.phase != MarketPhase::Active {
            return Err(Error::NotActive);
        }
        Ok(())
    }

    fn require_live(&self) -> Result<()> {
        if self.wrapper.retired {
            Err(Error::Retired)
        } else {
            Ok(())
        }
    }

    fn require_holder_wrappers(&self, holder: &HolderAssets, quantity: Amount) -> Result<()> {
        if holder.wrapper_atoms < quantity || self.wrapper.actual_supply < quantity {
            Err(Error::InsufficientWrappers)
        } else {
            Ok(())
        }
    }

    fn validate_holder_against_market(
        &self,
        market: &MarketLedger,
        holder: &HolderAssets,
    ) -> Result<()> {
        holder.validate(self.claim.basis.outcome_count, self.wrapper.actual_supply)?;
        let count = usize::from(self.claim.basis.outcome_count);
        let mut index = 0_usize;
        while index < count {
            let observed = holder.internal[index]
                .checked_add(self.vault.internal[index])
                .ok_or(Error::ArithmeticOverflow)?;
            if observed > market.base.total_supply[index] {
                return Err(Error::InvariantViolation);
            }
            index += 1;
        }
        Ok(())
    }
}

fn map_base_error(error: BaseError) -> Error {
    match error {
        BaseError::InvalidOutcomeCount | BaseError::InvalidPayoutCount => {
            Error::InvalidOutcomeCount
        }
        BaseError::InvalidPayoutIndex | BaseError::InvalidPayoutWeights => Error::InvalidWeights,
        BaseError::InvalidDenominator => Error::InvalidDenominator,
        BaseError::ZeroQuantity => Error::ZeroQuantity,
        BaseError::ArithmeticOverflow => Error::ArithmeticOverflow,
        BaseError::ArithmeticUnderflow => Error::ArithmeticUnderflow,
        BaseError::InsufficientBalance => Error::InsufficientEggs,
        BaseError::InsufficientCollateral => Error::InsufficientCash,
        BaseError::NotActive | BaseError::AlreadyResolved => Error::NotActive,
        BaseError::NotResolved => Error::NotResolved,
        BaseError::InvariantViolation => Error::InvariantViolation,
        BaseError::RemainderRequired => Error::InexactRedemption,
        BaseError::WrongResolutionMode => Error::InvalidWeights,
    }
}
