#![no_std]
#![forbid(unsafe_code)]

//! Checked research models for exact fractional redemption.
//!
//! This crate compares two policies without changing the production kernel or
//! Solana ABI:
//!
//! * exact-lot redemption refuses every non-integral claim; and
//! * persistent numerator credits turn every burned fractional claim into an
//!   explicit liability denominated in `1 / D` collateral atoms.
//!
//! The model is safe Rust, allocation-free, float-free, and fixed-width.  It
//! authenticates no account and performs no CPI; an adapter still owes signer,
//! PDA, token-supply, replay, rent, and exact post-CPI checks.

/// Maximum active native basis Eggs in one market.
pub const MAX_OUTCOMES: usize = 16;
/// Maximum credit objects in this bounded research state.
pub const MAX_CREDITS: usize = 16;

/// A fixed-width market or claimant identity.
pub type Id = [u8; 32];

/// Canonical refusal from the research model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidOutcomeCount,
    InvalidDenominator,
    InvalidWeights,
    InvalidIdentity,
    InvalidSlot,
    IdentityMismatch,
    ZeroQuantity,
    NonIntegralLot,
    InsufficientClaims,
    InsufficientCollateral,
    InsufficientCredit,
    CreditSlotOccupied,
    ArithmeticOverflow,
    InvariantViolation,
}

/// Result alias for total checked transitions.
pub type Result<T> = core::result::Result<T, Error>;

/// Whether the claim is held in a program-owned Position or as a bearer Egg.
///
/// The arithmetic is intentionally identical.  The adapter consequences are
/// not: an internal burn is a local store, while a bearer burn and collateral
/// payout must be atomic Token-2022 CPIs around the credit-account mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimKind {
    Internal,
    ExternalBearer,
}

/// One resolved integer simplex vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoutVector {
    pub outcome_count: u8,
    pub denominator: u64,
    pub weights: [u64; MAX_OUTCOMES],
}

impl PayoutVector {
    /// Validate the active prefix, canonical padding, and exact sum to `D`.
    pub fn validate(&self) -> Result<()> {
        let count = usize::from(self.outcome_count);
        if !(2..=MAX_OUTCOMES).contains(&count) {
            return Err(Error::InvalidOutcomeCount);
        }
        if self.denominator == 0 {
            return Err(Error::InvalidDenominator);
        }
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
                return Err(Error::InvalidWeights);
            }
            index += 1;
        }
        if sum != self.denominator {
            return Err(Error::InvalidWeights);
        }
        Ok(())
    }

    /// Exact unresolved claim numerator `sum_i supply_i * weight_i`.
    pub fn weighted_claim_numerator(&self, supplies: &[u64; MAX_OUTCOMES]) -> Result<u128> {
        self.validate()?;
        let mut total = 0_u128;
        let mut index = 0_usize;
        while index < usize::from(self.outcome_count) {
            let term = u128::from(supplies[index])
                .checked_mul(u128::from(self.weights[index]))
                .ok_or(Error::ArithmeticOverflow)?;
            total = total.checked_add(term).ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        Ok(total)
    }
}

/// Euclid's gcd, with `gcd(0, 0) = 0`.
pub const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// Least positive quantity whose payout is integral for one resolved weight.
///
/// A zero weight has lot one: losing claims can always burn for zero.
pub fn resolved_lot(denominator: u64, weight: u64) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::InvalidDenominator);
    }
    if weight > denominator {
        return Err(Error::InvalidWeights);
    }
    Ok(denominator / gcd(denominator, weight))
}

/// Least positive quantity integral under every reachable weight in a family.
///
/// This is `D / gcd(D, weights...)`.  Including zero weights is harmless.
/// An empty family means no extra restriction and returns one.
pub fn universal_lot(denominator: u64, reachable_weights: &[u64]) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::InvalidDenominator);
    }
    let mut common = denominator;
    let mut index = 0_usize;
    while index < reachable_weights.len() {
        let weight = reachable_weights[index];
        if weight > denominator {
            return Err(Error::InvalidWeights);
        }
        common = gcd(common, weight);
        index += 1;
    }
    Ok(denominator / common)
}

/// Least common lot across independently computed positive lots.
pub fn common_lot(lots: &[u64]) -> Result<u64> {
    let mut combined = 1_u64;
    let mut index = 0_usize;
    while index < lots.len() {
        let lot = lots[index];
        if lot == 0 {
            return Err(Error::InvalidDenominator);
        }
        combined = (combined / gcd(combined, lot))
            .checked_mul(lot)
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    Ok(combined)
}

/// Least common exact lot for every outcome in one fixed resolved vector.
///
/// This is `lcm_i D/gcd(D,w_i)`.  It can be much smaller than `D`; for
/// `[16,40,8]/64`, the per-outcome lots are `[4,8,8]` and the common lot is 8.
pub fn resolved_vector_common_lot(vector: &PayoutVector) -> Result<u64> {
    vector.validate()?;
    let mut combined = 1_u64;
    let mut index = 0_usize;
    while index < usize::from(vector.outcome_count) {
        let lot = resolved_lot(vector.denominator, vector.weights[index])?;
        combined = common_lot(&[combined, lot])?;
        index += 1;
    }
    Ok(combined)
}

/// Conservative lot when policy deliberately quantifies over the complete
/// integer-simplex family, rather than a smaller proved reachable family.
///
/// That family contains weight one whenever `D > 1`, hence its universal lot
/// is exactly `D`. This function does not claim a particular B-spline terms
/// instance actually reaches weight one; that is a separate compiler proof.
pub fn universal_integer_simplex_lot(denominator: u64) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::InvalidDenominator);
    }
    Ok(denominator)
}

/// Universal direct-redemption lot for a nonnegative structured coefficient
/// claim over any integer simplex vector of denominator `D`.
///
/// This is `lcm_i D/gcd(D, |a_i-a_0|)`, equivalently
/// `D/gcd(D, |a_i-a_0|...)`.  It is a research helper for the optional wrapper
/// architecture; native Egg `i` is the coefficient vector `e_i` and reduces
/// to lot `D`.
pub fn universal_structured_lot(
    denominator: u64,
    coefficients: &[u64; MAX_OUTCOMES],
    outcome_count: u8,
) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::InvalidDenominator);
    }
    let count = usize::from(outcome_count);
    if !(2..=MAX_OUTCOMES).contains(&count) {
        return Err(Error::InvalidOutcomeCount);
    }
    let anchor = coefficients[0];
    let mut common = denominator;
    let mut index = 1_usize;
    while index < count {
        common = gcd(common, anchor.abs_diff(coefficients[index]));
        index += 1;
    }
    let mut padding = count;
    while padding < MAX_OUTCOMES {
        if coefficients[padding] != 0 {
            return Err(Error::InvalidWeights);
        }
        padding += 1;
    }
    Ok(denominator / common)
}

/// Resolved direct-redemption lot for a nonnegative structured claim.
///
/// Only the dot product modulo `D` is needed.  Reducing after every term keeps
/// the calculation in checked `u128` even when the unreduced dot product would
/// not fit a machine word.
pub fn resolved_structured_lot(
    vector: &PayoutVector,
    coefficients: &[u64; MAX_OUTCOMES],
) -> Result<u64> {
    vector.validate()?;
    let denominator = u128::from(vector.denominator);
    let mut residue = 0_u128;
    let mut index = 0_usize;
    while index < usize::from(vector.outcome_count) {
        let term = u128::from(coefficients[index] % vector.denominator)
            .checked_mul(u128::from(vector.weights[index]))
            .ok_or(Error::ArithmeticOverflow)?;
        residue = (residue + (term % denominator)) % denominator;
        index += 1;
    }
    let mut padding = usize::from(vector.outcome_count);
    while padding < MAX_OUTCOMES {
        if coefficients[padding] != 0 {
            return Err(Error::InvalidWeights);
        }
        padding += 1;
    }
    Ok(vector.denominator / gcd(vector.denominator, residue as u64))
}

/// Require a quantity to respect a frozen nonzero lot.
pub fn require_lot(quantity: u64, lot: u64) -> Result<()> {
    if quantity == 0 {
        return Err(Error::ZeroQuantity);
    }
    if lot == 0 {
        return Err(Error::InvalidDenominator);
    }
    if !quantity.is_multiple_of(lot) {
        return Err(Error::NonIntegralLot);
    }
    Ok(())
}

/// Exact-lot market model: no credit state and no silent rounding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactLotMarket {
    pub payout: PayoutVector,
    pub remaining_claims: [u64; MAX_OUTCOMES],
    pub collateral_atoms: u64,
}

impl ExactLotMarket {
    /// Validate shape and resolved solvency.
    pub fn validate(&self) -> Result<()> {
        self.payout.validate()?;
        let left = u128::from(self.collateral_atoms)
            .checked_mul(u128::from(self.payout.denominator))
            .ok_or(Error::ArithmeticOverflow)?;
        let right = self
            .payout
            .weighted_claim_numerator(&self.remaining_claims)?;
        if left < right {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }

    /// Redeem exactly or refuse without changing any byte of model state.
    pub fn redeem(&mut self, _kind: ClaimKind, outcome: u8, quantity: u64) -> Result<u64> {
        self.validate()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let index = usize::from(outcome);
        if index >= usize::from(self.payout.outcome_count) {
            return Err(Error::InvalidOutcomeCount);
        }
        if self.remaining_claims[index] < quantity {
            return Err(Error::InsufficientClaims);
        }
        let numerator = u128::from(quantity)
            .checked_mul(u128::from(self.payout.weights[index]))
            .ok_or(Error::ArithmeticOverflow)?;
        let denominator = u128::from(self.payout.denominator);
        if numerator % denominator != 0 {
            return Err(Error::NonIntegralLot);
        }
        let payout =
            u64::try_from(numerator / denominator).map_err(|_| Error::ArithmeticOverflow)?;
        if self.collateral_atoms < payout {
            return Err(Error::InsufficientCollateral);
        }
        let mut next = *self;
        next.remaining_claims[index] = next.remaining_claims[index]
            .checked_sub(quantity)
            .ok_or(Error::InsufficientClaims)?;
        next.collateral_atoms = next
            .collateral_atoms
            .checked_sub(payout)
            .ok_or(Error::InsufficientCollateral)?;
        next.validate()?;
        *self = next;
        Ok(payout)
    }

    /// Recognize a direct bearer burn as a donation: liability falls and no
    /// collateral, cash, fee, bounty, or credit is created.
    pub fn direct_burn_donation(&mut self, outcome: u8, quantity: u64) -> Result<()> {
        self.validate()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let index = usize::from(outcome);
        if index >= usize::from(self.payout.outcome_count) {
            return Err(Error::InvalidOutcomeCount);
        }
        let mut next = *self;
        next.remaining_claims[index] = next.remaining_claims[index]
            .checked_sub(quantity)
            .ok_or(Error::InsufficientClaims)?;
        next.validate()?;
        *self = next;
        Ok(())
    }
}

/// Exact economic domain of a numerator credit.
///
/// `generation` is the immutable settlement/credit-accounting generation,
/// not a client timestamp.  A successor ABI or reopened market cannot merge
/// credit with an earlier generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditDomain {
    pub market: Id,
    pub denominator: u64,
    pub generation: u64,
}

impl CreditDomain {
    fn validate(&self) -> Result<()> {
        if self.market == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        if self.denominator == 0 {
            return Err(Error::InvalidDenominator);
        }
        Ok(())
    }
}

/// Full credit key.  The claimant is explicit; a transfer constructs a new
/// destination key under the exact same domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditKey {
    pub claimant: Id,
    pub domain: CreditDomain,
}

impl CreditKey {
    fn validate(&self) -> Result<()> {
        if self.claimant == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        self.domain.validate()
    }
}

/// One fixed-width persistent sub-atom liability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditAccount {
    pub initialized: bool,
    pub key: CreditKey,
    /// Canonical residue numerator, always strictly less than `D`.
    pub numerator: u64,
}

impl CreditAccount {
    /// Canonical empty slot in the bounded research state.
    pub const EMPTY: Self = Self {
        initialized: false,
        key: CreditKey {
            claimant: [0; 32],
            domain: CreditDomain {
                market: [0; 32],
                denominator: 0,
                generation: 0,
            },
        },
        numerator: 0,
    };

    fn validate_empty(&self) -> Result<()> {
        if *self != Self::EMPTY {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }

    fn validate_initialized(&self) -> Result<()> {
        self.key.validate()?;
        if self.numerator >= self.key.domain.denominator {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }
}

/// Result of a claim burn or credit aggregation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditEffect {
    /// Whole collateral atoms paid now.
    pub paid_atoms: u64,
    /// Canonical numerator remaining in the destination credit.
    pub destination_numerator: u64,
}

/// Persistent-credit market with one market-level liability total.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditMarket {
    pub domain: CreditDomain,
    pub payout: PayoutVector,
    pub remaining_claims: [u64; MAX_OUTCOMES],
    pub collateral_atoms: u64,
    /// Sum of all initialized credit numerators.  This is persisted market
    /// state in a production design, not a value inferred by scanning users.
    pub credit_numerator_total: u128,
    pub credits: [CreditAccount; MAX_CREDITS],
}

impl CreditMarket {
    /// Validate identities, the aggregate credit owner, and
    /// `D*C >= weighted remaining claims + all credits`.
    pub fn validate(&self) -> Result<()> {
        self.domain.validate()?;
        self.payout.validate()?;
        if self.domain.denominator != self.payout.denominator {
            return Err(Error::IdentityMismatch);
        }
        let mut observed_credit = 0_u128;
        let mut index = 0_usize;
        while index < MAX_CREDITS {
            let credit = self.credits[index];
            if credit.initialized {
                credit.validate_initialized()?;
                if credit.key.domain != self.domain {
                    return Err(Error::IdentityMismatch);
                }
                let mut prior = 0_usize;
                while prior < index {
                    if self.credits[prior].initialized && self.credits[prior].key == credit.key {
                        // A canonical PDA gives one semantic owner to one full
                        // key. Two slots with the same key would be parallel
                        // truths even if their numerators happened to sum.
                        return Err(Error::CreditSlotOccupied);
                    }
                    prior += 1;
                }
                observed_credit = observed_credit
                    .checked_add(u128::from(credit.numerator))
                    .ok_or(Error::ArithmeticOverflow)?;
            } else {
                credit.validate_empty()?;
            }
            index += 1;
        }
        if observed_credit != self.credit_numerator_total {
            return Err(Error::InvariantViolation);
        }
        let backing = u128::from(self.collateral_atoms)
            .checked_mul(u128::from(self.payout.denominator))
            .ok_or(Error::ArithmeticOverflow)?;
        let claims = self
            .payout
            .weighted_claim_numerator(&self.remaining_claims)?;
        let liabilities = claims
            .checked_add(self.credit_numerator_total)
            .ok_or(Error::ArithmeticOverflow)?;
        if backing < liabilities {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }

    /// Numerator slack above all remaining claims and credits.
    pub fn slack_numerator(&self) -> Result<u128> {
        self.validate()?;
        let backing = u128::from(self.collateral_atoms)
            .checked_mul(u128::from(self.payout.denominator))
            .ok_or(Error::ArithmeticOverflow)?;
        let claims = self
            .payout
            .weighted_claim_numerator(&self.remaining_claims)?;
        backing
            .checked_sub(
                claims
                    .checked_add(self.credit_numerator_total)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::InvariantViolation)
    }

    fn validate_slot(slot: usize) -> Result<()> {
        if slot >= MAX_CREDITS {
            return Err(Error::InvalidSlot);
        }
        Ok(())
    }

    fn require_key_domain(&self, key: CreditKey) -> Result<()> {
        key.validate()?;
        if key.domain != self.domain {
            return Err(Error::IdentityMismatch);
        }
        Ok(())
    }

    /// Burn arbitrary-quantity claims, pay only whole atoms, and preserve the
    /// exact remainder in a claimant-bound credit.
    ///
    /// Different outcomes may target the same exact `CreditKey`; their
    /// numerators aggregate before one division.  Every check is made on a
    /// prospective copy, so any refusal leaves the original state unchanged.
    pub fn redeem_to_credit(
        &mut self,
        _kind: ClaimKind,
        slot: usize,
        key: CreditKey,
        outcome: u8,
        quantity: u64,
    ) -> Result<CreditEffect> {
        self.validate()?;
        Self::validate_slot(slot)?;
        self.require_key_domain(key)?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let outcome_index = usize::from(outcome);
        if outcome_index >= usize::from(self.payout.outcome_count) {
            return Err(Error::InvalidOutcomeCount);
        }
        if self.remaining_claims[outcome_index] < quantity {
            return Err(Error::InsufficientClaims);
        }
        let existing = self.credits[slot];
        if existing.initialized && existing.key != key {
            return Err(Error::IdentityMismatch);
        }
        let prior = if existing.initialized {
            existing.numerator
        } else {
            0
        };
        let claim_numerator = u128::from(quantity)
            .checked_mul(u128::from(self.payout.weights[outcome_index]))
            .ok_or(Error::ArithmeticOverflow)?;
        let accumulated = u128::from(prior)
            .checked_add(claim_numerator)
            .ok_or(Error::ArithmeticOverflow)?;
        let denominator = u128::from(self.payout.denominator);
        let paid =
            u64::try_from(accumulated / denominator).map_err(|_| Error::ArithmeticOverflow)?;
        let residue =
            u64::try_from(accumulated % denominator).map_err(|_| Error::ArithmeticOverflow)?;
        if self.collateral_atoms < paid {
            return Err(Error::InsufficientCollateral);
        }

        let mut next = *self;
        next.remaining_claims[outcome_index] = next.remaining_claims[outcome_index]
            .checked_sub(quantity)
            .ok_or(Error::InsufficientClaims)?;
        next.collateral_atoms = next
            .collateral_atoms
            .checked_sub(paid)
            .ok_or(Error::InsufficientCollateral)?;
        next.credits[slot] = CreditAccount {
            initialized: true,
            key,
            numerator: residue,
        };
        next.credit_numerator_total = next
            .credit_numerator_total
            .checked_sub(u128::from(prior))
            .and_then(|value| value.checked_add(u128::from(residue)))
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate()?;
        *self = next;
        Ok(CreditEffect {
            paid_atoms: paid,
            destination_numerator: residue,
        })
    }

    /// Transfer numerator units and merge them into a destination credit.
    ///
    /// The source and destination may bind different claimants, but both full
    /// keys are explicit and their market/denominator/generation domain must
    /// match exactly.  If aggregation crosses `D`, the whole atom is paid to
    /// the destination claimant and removed from collateral in the same atomic
    /// transition.  No numerator is discarded.
    pub fn transfer_credit(
        &mut self,
        source_slot: usize,
        expected_source: CreditKey,
        destination_slot: usize,
        expected_destination: CreditKey,
        numerator: u64,
    ) -> Result<CreditEffect> {
        self.validate()?;
        Self::validate_slot(source_slot)?;
        Self::validate_slot(destination_slot)?;
        if source_slot == destination_slot {
            return Err(Error::InvalidSlot);
        }
        self.require_key_domain(expected_source)?;
        self.require_key_domain(expected_destination)?;
        if numerator == 0 {
            return Err(Error::ZeroQuantity);
        }
        let source = self.credits[source_slot];
        if !source.initialized || source.key != expected_source {
            return Err(Error::IdentityMismatch);
        }
        if source.numerator < numerator {
            return Err(Error::InsufficientCredit);
        }
        let destination = self.credits[destination_slot];
        if destination.initialized && destination.key != expected_destination {
            return Err(Error::IdentityMismatch);
        }
        let destination_before = if destination.initialized {
            destination.numerator
        } else {
            0
        };
        let accumulated = u128::from(destination_before)
            .checked_add(u128::from(numerator))
            .ok_or(Error::ArithmeticOverflow)?;
        let denominator = u128::from(self.domain.denominator);
        let paid =
            u64::try_from(accumulated / denominator).map_err(|_| Error::ArithmeticOverflow)?;
        let destination_after =
            u64::try_from(accumulated % denominator).map_err(|_| Error::ArithmeticOverflow)?;
        if self.collateral_atoms < paid {
            return Err(Error::InsufficientCollateral);
        }

        let mut next = *self;
        next.credits[source_slot].numerator = next.credits[source_slot]
            .numerator
            .checked_sub(numerator)
            .ok_or(Error::InsufficientCredit)?;
        next.credits[destination_slot] = CreditAccount {
            initialized: true,
            key: expected_destination,
            numerator: destination_after,
        };
        next.collateral_atoms = next
            .collateral_atoms
            .checked_sub(paid)
            .ok_or(Error::InsufficientCollateral)?;
        next.credit_numerator_total = next
            .credit_numerator_total
            .checked_sub(u128::from(paid) * denominator)
            .ok_or(Error::InvariantViolation)?;
        next.validate()?;
        *self = next;
        Ok(CreditEffect {
            paid_atoms: paid,
            destination_numerator: destination_after,
        })
    }

    /// Move an entire source residue into the destination.
    pub fn merge_credit(
        &mut self,
        source_slot: usize,
        expected_source: CreditKey,
        destination_slot: usize,
        expected_destination: CreditKey,
    ) -> Result<CreditEffect> {
        Self::validate_slot(source_slot)?;
        let source = self.credits[source_slot];
        if !source.initialized || source.key != expected_source {
            return Err(Error::IdentityMismatch);
        }
        self.transfer_credit(
            source_slot,
            expected_source,
            destination_slot,
            expected_destination,
            source.numerator,
        )
    }

    /// Recognize a direct bearer burn as a donation.  Credits and collateral
    /// are untouched; the exact slack increase is `quantity * weight`.
    pub fn direct_burn_donation(&mut self, outcome: u8, quantity: u64) -> Result<()> {
        self.validate()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let index = usize::from(outcome);
        if index >= usize::from(self.payout.outcome_count) {
            return Err(Error::InvalidOutcomeCount);
        }
        let mut next = *self;
        next.remaining_claims[index] = next.remaining_claims[index]
            .checked_sub(quantity)
            .ok_or(Error::InsufficientClaims)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Terminal accounting facts.  Whole credit atoms remain aggregatable;
    /// `irreducible_credit_numerator` cannot be paid exactly in collateral
    /// atoms without future same-domain aggregation or a separately funded
    /// rounding policy.
    pub fn terminal_facts(&self) -> Result<TerminalFacts> {
        self.validate()?;
        Ok(TerminalFacts {
            weighted_claim_numerator: self
                .payout
                .weighted_claim_numerator(&self.remaining_claims)?,
            credit_numerator_total: self.credit_numerator_total,
            aggregatable_credit_atoms: self.credit_numerator_total
                / u128::from(self.domain.denominator),
            irreducible_credit_numerator: u64::try_from(
                self.credit_numerator_total % u128::from(self.domain.denominator),
            )
            .map_err(|_| Error::ArithmeticOverflow)?,
            collateral_atoms: self.collateral_atoms,
        })
    }
}

/// Exact terminal decomposition; this type deliberately does not declare a
/// treasury or operator recipient for residual collateral.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalFacts {
    pub weighted_claim_numerator: u128,
    pub credit_numerator_total: u128,
    pub aggregatable_credit_atoms: u128,
    pub irreducible_credit_numerator: u64,
    pub collateral_atoms: u64,
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id {
        [byte; 32]
    }

    fn vector(denominator: u64, active: &[u64]) -> PayoutVector {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[..active.len()].copy_from_slice(active);
        PayoutVector {
            outcome_count: active.len() as u8,
            denominator,
            weights,
        }
    }

    fn key(claimant: u8, denominator: u64, generation: u64) -> CreditKey {
        CreditKey {
            claimant: id(claimant),
            domain: CreditDomain {
                market: id(99),
                denominator,
                generation,
            },
        }
    }

    fn credit_market(denominator: u64, weights: &[u64], supply: u64) -> CreditMarket {
        let payout = vector(denominator, weights);
        let mut remaining_claims = [0_u64; MAX_OUTCOMES];
        let mut index = 0_usize;
        while index < weights.len() {
            remaining_claims[index] = supply;
            index += 1;
        }
        CreditMarket {
            domain: CreditDomain {
                market: id(99),
                denominator,
                generation: 7,
            },
            payout,
            remaining_claims,
            collateral_atoms: supply,
            credit_numerator_total: 0,
            credits: [CreditAccount::EMPTY; MAX_CREDITS],
        }
    }

    #[test]
    fn resolved_and_universal_lots_are_minimal_exhaustively() {
        let fixed = vector(64, &[16, 40, 8]);
        assert_eq!(resolved_lot(64, 16), Ok(4));
        assert_eq!(resolved_lot(64, 40), Ok(8));
        assert_eq!(resolved_lot(64, 8), Ok(8));
        assert_eq!(resolved_vector_common_lot(&fixed), Ok(8));

        let mut denominator = 1_u64;
        while denominator <= 24 {
            let mut weight = 0_u64;
            while weight <= denominator {
                let lot = resolved_lot(denominator, weight).unwrap();
                assert_eq!(
                    (u128::from(lot) * u128::from(weight)) % u128::from(denominator),
                    0
                );
                let mut smaller = 1_u64;
                while smaller < lot {
                    assert_ne!(
                        (u128::from(smaller) * u128::from(weight)) % u128::from(denominator),
                        0
                    );
                    smaller += 1;
                }
                weight += 1;
            }

            let mut a = 0_u64;
            while a <= denominator {
                let mut b = 0_u64;
                while b <= denominator {
                    let family = [a, b];
                    let lot = universal_lot(denominator, &family).unwrap();
                    for candidate_weight in family {
                        assert_eq!(
                            (u128::from(lot) * u128::from(candidate_weight))
                                % u128::from(denominator),
                            0
                        );
                    }
                    let mut smaller = 1_u64;
                    while smaller < lot {
                        assert!(family.iter().any(|candidate_weight| {
                            (u128::from(smaller) * u128::from(*candidate_weight))
                                % u128::from(denominator)
                                != 0
                        }));
                        smaller += 1;
                    }
                    b += 1;
                }
                a += 1;
            }
            assert_eq!(
                universal_lot(denominator, &[0, 1, denominator]).unwrap(),
                denominator
            );
            assert_eq!(
                universal_integer_simplex_lot(denominator).unwrap(),
                denominator
            );
            denominator += 1;
        }
    }

    #[test]
    fn universal_structured_formula_is_minimal_on_small_simplexes() {
        let mut denominator = 1_u64;
        while denominator <= 10 {
            let mut a0 = 0_u64;
            while a0 <= 6 {
                let mut a1 = 0_u64;
                while a1 <= 6 {
                    let mut a2 = 0_u64;
                    while a2 <= 6 {
                        let mut coefficients = [0_u64; MAX_OUTCOMES];
                        coefficients[0] = a0;
                        coefficients[1] = a1;
                        coefficients[2] = a2;
                        let lot = universal_structured_lot(denominator, &coefficients, 3).unwrap();
                        let mut w0 = 0_u64;
                        while w0 <= denominator {
                            let mut w1 = 0_u64;
                            while w1 <= denominator - w0 {
                                let w2 = denominator - w0 - w1;
                                let dot = u128::from(a0) * u128::from(w0)
                                    + u128::from(a1) * u128::from(w1)
                                    + u128::from(a2) * u128::from(w2);
                                assert_eq!(u128::from(lot) * dot % u128::from(denominator), 0);
                                w1 += 1;
                            }
                            w0 += 1;
                        }
                        let mut smaller = 1_u64;
                        while smaller < lot {
                            let mut witness = false;
                            let mut w0 = 0_u64;
                            while w0 <= denominator {
                                let mut w1 = 0_u64;
                                while w1 <= denominator - w0 {
                                    let w2 = denominator - w0 - w1;
                                    let dot = u128::from(a0) * u128::from(w0)
                                        + u128::from(a1) * u128::from(w1)
                                        + u128::from(a2) * u128::from(w2);
                                    witness |=
                                        u128::from(smaller) * dot % u128::from(denominator) != 0;
                                    w1 += 1;
                                }
                                w0 += 1;
                            }
                            assert!(witness);
                            smaller += 1;
                        }
                        a2 += 1;
                    }
                    a1 += 1;
                }
                a0 += 1;
            }
            denominator += 1;
        }
    }

    #[test]
    fn exact_lot_refusal_is_atomic_and_direct_burn_is_only_a_donation() {
        let mut market = ExactLotMarket {
            payout: vector(6, &[1, 2, 3]),
            remaining_claims: {
                let mut values = [0; MAX_OUTCOMES];
                values[..3].copy_from_slice(&[6, 6, 6]);
                values
            },
            collateral_atoms: 6,
        };
        market.validate().unwrap();
        let before = market;
        assert_eq!(
            market.redeem(ClaimKind::Internal, 0, 1),
            Err(Error::NonIntegralLot)
        );
        assert_eq!(market, before);
        assert_eq!(market.redeem(ClaimKind::ExternalBearer, 1, 3), Ok(1));
        let before_burn = u128::from(market.collateral_atoms) * 6
            - market
                .payout
                .weighted_claim_numerator(&market.remaining_claims)
                .unwrap();
        market.direct_burn_donation(2, 2).unwrap();
        let after_burn = u128::from(market.collateral_atoms) * 6
            - market
                .payout
                .weighted_claim_numerator(&market.remaining_claims)
                .unwrap();
        assert_eq!(after_burn - before_burn, 6);
    }

    #[test]
    fn mixed_outcomes_aggregate_without_losing_a_numerator() {
        let mut market = credit_market(6, &[1, 2, 3], 1);
        let claimant = key(1, 6, 7);
        market.validate().unwrap();
        assert_eq!(
            market
                .redeem_to_credit(ClaimKind::Internal, 0, claimant, 0, 1)
                .unwrap(),
            CreditEffect {
                paid_atoms: 0,
                destination_numerator: 1
            }
        );
        assert_eq!(
            market
                .redeem_to_credit(ClaimKind::ExternalBearer, 0, claimant, 1, 1)
                .unwrap(),
            CreditEffect {
                paid_atoms: 0,
                destination_numerator: 3
            }
        );
        assert_eq!(
            market
                .redeem_to_credit(ClaimKind::Internal, 0, claimant, 2, 1)
                .unwrap(),
            CreditEffect {
                paid_atoms: 1,
                destination_numerator: 0
            }
        );
        assert_eq!(market.collateral_atoms, 0);
        assert_eq!(market.credit_numerator_total, 0);
        assert_eq!(
            market
                .payout
                .weighted_claim_numerator(&market.remaining_claims)
                .unwrap(),
            0
        );
        market.validate().unwrap();
    }

    #[test]
    fn transfer_and_merge_require_exact_domain_and_preserve_conservation() {
        let mut market = credit_market(10, &[3, 7], 2);
        let alice = key(1, 10, 7);
        let bob = key(2, 10, 7);
        market
            .redeem_to_credit(ClaimKind::Internal, 0, alice, 0, 1)
            .unwrap();
        market
            .redeem_to_credit(ClaimKind::ExternalBearer, 1, bob, 1, 1)
            .unwrap();
        assert_eq!(market.credit_numerator_total, 10);
        let slack = market.slack_numerator().unwrap();
        let effect = market.merge_credit(0, alice, 1, bob).unwrap();
        assert_eq!(
            effect,
            CreditEffect {
                paid_atoms: 1,
                destination_numerator: 0
            }
        );
        assert_eq!(market.credit_numerator_total, 0);
        assert_eq!(market.slack_numerator().unwrap(), slack);

        let wrong_generation = key(2, 10, 8);
        let before = market;
        assert_eq!(
            market.transfer_credit(1, bob, 2, wrong_generation, 1),
            Err(Error::IdentityMismatch)
        );
        assert_eq!(market, before);
    }

    #[test]
    fn fragmentation_and_reaggregation_match_one_shot_redemption() {
        let mut one_shot = credit_market(13, &[5, 8], 13);
        let alice = key(1, 13, 7);
        let effect = one_shot
            .redeem_to_credit(ClaimKind::Internal, 0, alice, 0, 13)
            .unwrap();
        assert_eq!(
            effect,
            CreditEffect {
                paid_atoms: 5,
                destination_numerator: 0
            }
        );

        let mut fragmented = credit_market(13, &[5, 8], 13);
        let mut paid = 0_u64;
        let mut quantity = 0_u64;
        while quantity < 13 {
            paid += fragmented
                .redeem_to_credit(ClaimKind::Internal, 0, alice, 0, 1)
                .unwrap()
                .paid_atoms;
            quantity += 1;
        }
        assert_eq!(paid, 5);
        assert_eq!(fragmented.credits[0].numerator, 0);
        assert_eq!(fragmented.remaining_claims, one_shot.remaining_claims);
        assert_eq!(fragmented.collateral_atoms, one_shot.collateral_atoms);
        assert_eq!(
            fragmented.credit_numerator_total,
            one_shot.credit_numerator_total
        );
    }

    #[test]
    fn credit_conservation_is_exhaustive_on_small_complete_sets() {
        let mut denominator = 1_u64;
        while denominator <= 16 {
            let mut first_weight = 0_u64;
            while first_weight <= denominator {
                let mut quantity = 1_u64;
                while quantity <= 12 {
                    let claimant = key(1, denominator, 7);
                    let mut forward = credit_market(
                        denominator,
                        &[first_weight, denominator - first_weight],
                        quantity,
                    );
                    let mut reverse = forward;
                    let mut paid_forward = 0_u64;
                    let mut paid_reverse = 0_u64;
                    let mut unit = 0_u64;
                    while unit < quantity {
                        paid_forward += forward
                            .redeem_to_credit(ClaimKind::Internal, 0, claimant, 0, 1)
                            .unwrap()
                            .paid_atoms;
                        paid_forward += forward
                            .redeem_to_credit(ClaimKind::ExternalBearer, 0, claimant, 1, 1)
                            .unwrap()
                            .paid_atoms;
                        paid_reverse += reverse
                            .redeem_to_credit(ClaimKind::ExternalBearer, 0, claimant, 1, 1)
                            .unwrap()
                            .paid_atoms;
                        paid_reverse += reverse
                            .redeem_to_credit(ClaimKind::Internal, 0, claimant, 0, 1)
                            .unwrap()
                            .paid_atoms;
                        forward.validate().unwrap();
                        reverse.validate().unwrap();
                        unit += 1;
                    }
                    assert_eq!(paid_forward, quantity);
                    assert_eq!(paid_reverse, quantity);
                    assert_eq!(forward, reverse);
                    assert_eq!(forward.collateral_atoms, 0);
                    assert_eq!(forward.credit_numerator_total, 0);
                    assert_eq!(forward.remaining_claims, [0; MAX_OUTCOMES]);
                    quantity += 1;
                }
                first_weight += 1;
            }
            denominator += 1;
        }
    }

    #[test]
    fn terminal_sub_atom_is_reported_not_erased() {
        let mut market = credit_market(7, &[1, 6], 1);
        let alice = key(1, 7, 7);
        market
            .redeem_to_credit(ClaimKind::ExternalBearer, 0, alice, 0, 1)
            .unwrap();
        market.direct_burn_donation(1, 1).unwrap();
        let facts = market.terminal_facts().unwrap();
        assert_eq!(facts.weighted_claim_numerator, 0);
        assert_eq!(facts.credit_numerator_total, 1);
        assert_eq!(facts.aggregatable_credit_atoms, 0);
        assert_eq!(facts.irreducible_credit_numerator, 1);
        assert_eq!(facts.collateral_atoms, 1);
    }

    #[test]
    fn aggregate_credit_total_is_a_required_semantic_owner() {
        let mut market = credit_market(5, &[2, 3], 1);
        let alice = key(1, 5, 7);
        market
            .redeem_to_credit(ClaimKind::Internal, 0, alice, 0, 1)
            .unwrap();
        let mut corrupted = market;
        corrupted.credit_numerator_total += 1;
        assert_eq!(corrupted.validate(), Err(Error::InvariantViolation));
    }

    #[test]
    fn one_full_credit_key_has_one_semantic_owner() {
        let mut market = credit_market(5, &[2, 3], 1);
        let alice = key(1, 5, 7);
        market.credits[0] = CreditAccount {
            initialized: true,
            key: alice,
            numerator: 1,
        };
        market.credits[1] = CreditAccount {
            initialized: true,
            key: alice,
            numerator: 1,
        };
        market.credit_numerator_total = 2;
        assert_eq!(market.validate(), Err(Error::CreditSlotOccupied));
    }

    #[test]
    fn direct_burn_increases_slack_by_exact_weighted_numerator() {
        let mut market = credit_market(11, &[4, 7], 9);
        let before = market.slack_numerator().unwrap();
        market.direct_burn_donation(1, 3).unwrap();
        let after = market.slack_numerator().unwrap();
        assert_eq!(after - before, 21);
        assert_eq!(market.credit_numerator_total, 0);
        assert_eq!(market.collateral_atoms, 9);
    }

    #[test]
    fn hostile_arithmetic_boundaries_are_checked() {
        let market = CreditMarket {
            domain: CreditDomain {
                market: id(99),
                denominator: u64::MAX,
                generation: 7,
            },
            payout: vector(u64::MAX, &[u64::MAX - 1, 1]),
            remaining_claims: {
                let mut values = [0; MAX_OUTCOMES];
                values[0] = u64::MAX;
                values[1] = u64::MAX;
                values
            },
            collateral_atoms: u64::MAX,
            credit_numerator_total: 0,
            credits: [CreditAccount::EMPTY; MAX_CREDITS],
        };
        // This complete-set shape fits exactly even at the numeric boundary.
        market.validate().unwrap();

        let mut uneven = market;
        uneven.payout = vector(u64::MAX, &[u64::MAX / 2, u64::MAX - (u64::MAX / 2)]);
        uneven.remaining_claims[0] = u64::MAX;
        uneven.remaining_claims[1] = u64::MAX;
        uneven.validate().unwrap();

        // Shape validation itself is checked: two individually in-range
        // maximal weights cannot wrap their way into a valid sum-to-D vector.
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = u64::MAX;
        weights[1] = u64::MAX;
        let overflowing = CreditMarket {
            payout: PayoutVector {
                outcome_count: 2,
                denominator: u64::MAX,
                weights,
            },
            remaining_claims: [u64::MAX; MAX_OUTCOMES],
            ..market
        };
        assert_eq!(overflowing.validate(), Err(Error::ArithmeticOverflow));
    }

    #[test]
    fn deterministic_random_campaign_preserves_exact_liability() {
        let mut seed = 0x7d35_21ab_c901_7741_u64;
        let mut case = 0_u64;
        while case < 2_000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let denominator = 2 + (seed % 63);
            let weight0 = 1 + ((seed >> 8) % (denominator - 1));
            let quantity = 1 + ((seed >> 16) % 31);
            let mut market =
                credit_market(denominator, &[weight0, denominator - weight0], quantity);
            let claimant0 = key(1, denominator, 7);
            let claimant1 = key(2, denominator, 7);
            let initial_slack = market.slack_numerator().unwrap();
            let mut paid = 0_u64;
            let mut remaining = quantity;
            while remaining > 0 {
                let chunk = 1 + ((seed ^ remaining) % remaining);
                let slot = (remaining as usize) & 1;
                let claimant = if slot == 0 { claimant0 } else { claimant1 };
                paid = paid
                    .checked_add(
                        market
                            .redeem_to_credit(ClaimKind::Internal, slot, claimant, 0, chunk)
                            .unwrap()
                            .paid_atoms,
                    )
                    .unwrap();
                remaining -= chunk;
            }
            if market.credits[0].numerator != 0 && market.credits[1].numerator != 0 {
                let effect = market.merge_credit(0, claimant0, 1, claimant1).unwrap();
                paid += effect.paid_atoms;
            }
            assert_eq!(market.slack_numerator().unwrap(), initial_slack);
            let burned_num = u128::from(quantity) * u128::from(weight0);
            assert_eq!(
                u128::from(paid) * u128::from(denominator) + market.credit_numerator_total,
                burned_num
            );
            market.validate().unwrap();
            case += 1;
        }
    }
}
