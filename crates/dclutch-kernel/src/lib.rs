#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Total, allocation-free liability transitions for the dClutch Market Core.
//!
//! This bootstrap module deliberately implements only categorical complete-set
//! accounting. It contains no Solana accounts, token operations, oracle SDK,
//! execution venue, fees, rent, or operator policy.

/// Maximum categorical width in the first measured implementation profile.
///
/// This is a provisional profile bound, not a conceptual protocol limit.
pub const MAX_OUTCOMES: usize = 16;

/// Minimum number of outcomes in a categorical market.
pub const MIN_OUTCOMES: usize = 2;

/// Explicit refusal returned by a kernel transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The compile-time outcome width is outside the selected profile.
    InvalidOutcomeCount,
    /// A supplied outcome index is outside the active width.
    InvalidOutcome,
    /// A quantity that must move value was zero.
    ZeroQuantity,
    /// The requested transition is not admitted in the current phase.
    InvalidPhase,
    /// An addition or subtraction would leave the exact integer domain.
    ArithmeticOverflow,
    /// The requested burn or merge exceeds an outstanding supply.
    InsufficientSupply,
    /// Hoard collateral does not cover the phase-specific maximum liability.
    Insolvent,
}

/// Kernel result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// Current categorical Market phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// Complete sets may be split or merged; no terminal result exists.
    Open,
    /// One exact categorical outcome is terminal.
    Resolved {
        /// Winning outcome index.
        winner: usize,
    },
}

/// Aggregate categorical liability state owned by the Market Core.
///
/// `supply[i]` is the conservative total outstanding amount of native claim
/// `i`, independent of whether units are held internally or materialized by an
/// adapter. `hoard_atoms` is claimant-backing collateral only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalLedger<const N: usize> {
    hoard_atoms: u64,
    supply: [u64; N],
    phase: Phase,
}

impl<const N: usize> CategoricalLedger<N> {
    /// Construct an empty open ledger after validating the selected width.
    pub fn new() -> Result<Self> {
        validate_width::<N>()?;
        Ok(Self {
            hoard_atoms: 0,
            supply: [0; N],
            phase: Phase::Open,
        })
    }

    /// Return claimant-backing collateral atoms.
    pub const fn hoard_atoms(&self) -> u64 {
        self.hoard_atoms
    }

    /// Return the exact conservative supply vector.
    pub const fn supply(&self) -> &[u64; N] {
        &self.supply
    }

    /// Return the current phase.
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// Validate width, terminal index, and phase-specific solvency.
    pub fn validate(&self) -> Result<()> {
        validate_width::<N>()?;
        let required = match self.phase {
            Phase::Open => max_supply(&self.supply),
            Phase::Resolved { winner } => {
                if winner >= N {
                    return Err(Error::InvalidOutcome);
                }
                self.supply
                    .get(winner)
                    .copied()
                    .ok_or(Error::InvalidOutcome)?
            }
        };
        if self.hoard_atoms < required {
            return Err(Error::Insolvent);
        }
        Ok(())
    }

    /// Deposit collateral and issue one complete categorical set.
    pub fn split_complete_set(&mut self, quantity: u64) -> Result<()> {
        self.require_open_nonzero(quantity)?;
        let next_hoard = self
            .hoard_atoms
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut next_supply = self.supply;
        for amount in &mut next_supply {
            *amount = amount
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        let next = Self {
            hoard_atoms: next_hoard,
            supply: next_supply,
            phase: self.phase,
        };
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Burn one complete categorical set and withdraw its backing collateral.
    pub fn merge_complete_set(&mut self, quantity: u64) -> Result<()> {
        self.require_open_nonzero(quantity)?;
        let mut next_supply = self.supply;
        for amount in &mut next_supply {
            *amount = amount
                .checked_sub(quantity)
                .ok_or(Error::InsufficientSupply)?;
        }
        let next_hoard = self
            .hoard_atoms
            .checked_sub(quantity)
            .ok_or(Error::Insolvent)?;
        let next = Self {
            hoard_atoms: next_hoard,
            supply: next_supply,
            phase: self.phase,
        };
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Freeze one exact winning outcome.
    pub fn resolve(&mut self, winner: usize) -> Result<()> {
        self.validate()?;
        if self.phase != Phase::Open {
            return Err(Error::InvalidPhase);
        }
        if winner >= N {
            return Err(Error::InvalidOutcome);
        }
        let next = Self {
            hoard_atoms: self.hoard_atoms,
            supply: self.supply,
            phase: Phase::Resolved { winner },
        };
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Burn resolved claims and return their exact collateral payout.
    ///
    /// A winning categorical claim pays one collateral atom per claim atom. A
    /// losing claim pays zero. This transition names no rounding boundary
    /// because the categorical payout is integral.
    pub fn redeem(&mut self, outcome: usize, quantity: u64) -> Result<u64> {
        self.validate()?;
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let Phase::Resolved { winner } = self.phase else {
            return Err(Error::InvalidPhase);
        };
        if outcome >= N {
            return Err(Error::InvalidOutcome);
        }
        let mut next_supply = self.supply;
        let selected = next_supply.get_mut(outcome).ok_or(Error::InvalidOutcome)?;
        *selected = selected
            .checked_sub(quantity)
            .ok_or(Error::InsufficientSupply)?;
        let payout = if outcome == winner { quantity } else { 0 };
        let next_hoard = self
            .hoard_atoms
            .checked_sub(payout)
            .ok_or(Error::Insolvent)?;
        let next = Self {
            hoard_atoms: next_hoard,
            supply: next_supply,
            phase: self.phase,
        };
        next.validate()?;
        *self = next;
        Ok(payout)
    }

    fn require_open_nonzero(&self, quantity: u64) -> Result<()> {
        self.validate()?;
        if self.phase != Phase::Open {
            return Err(Error::InvalidPhase);
        }
        if quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        Ok(())
    }
}

fn validate_width<const N: usize>() -> Result<()> {
    if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&N) {
        return Err(Error::InvalidOutcomeCount);
    }
    Ok(())
}

fn max_supply<const N: usize>(supply: &[u64; N]) -> u64 {
    let mut maximum = 0u64;
    for amount in supply {
        if *amount > maximum {
            maximum = *amount;
        }
    }
    maximum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_set_round_trip_preserves_empty_state() -> Result<()> {
        let mut ledger = CategoricalLedger::<3>::new()?;
        ledger.split_complete_set(50)?;
        assert_eq!(ledger.hoard_atoms(), 50);
        assert_eq!(ledger.supply(), &[50, 50, 50]);
        ledger.merge_complete_set(50)?;
        assert_eq!(ledger, CategoricalLedger::<3>::new()?);
        Ok(())
    }

    #[test]
    fn resolved_redemption_pays_only_the_winner() -> Result<()> {
        let mut ledger = CategoricalLedger::<3>::new()?;
        ledger.split_complete_set(50)?;
        ledger.resolve(1)?;
        assert_eq!(ledger.redeem(0, 50), Ok(0));
        assert_eq!(ledger.redeem(2, 50), Ok(0));
        assert_eq!(ledger.redeem(1, 50), Ok(50));
        assert_eq!(ledger.hoard_atoms(), 0);
        assert_eq!(ledger.supply(), &[0, 0, 0]);
        Ok(())
    }

    #[test]
    fn hostile_width_phase_quantity_and_supply_refuse_without_mutation() -> Result<()> {
        assert_eq!(
            CategoricalLedger::<1>::new(),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            CategoricalLedger::<17>::new(),
            Err(Error::InvalidOutcomeCount)
        );

        let mut ledger = CategoricalLedger::<2>::new()?;
        assert_eq!(ledger.split_complete_set(0), Err(Error::ZeroQuantity));
        assert_eq!(ledger.merge_complete_set(1), Err(Error::InsufficientSupply));
        ledger.split_complete_set(7)?;
        let before = ledger;
        assert_eq!(ledger.resolve(2), Err(Error::InvalidOutcome));
        assert_eq!(ledger, before);
        ledger.resolve(0)?;
        let resolved = ledger;
        assert_eq!(ledger.split_complete_set(1), Err(Error::InvalidPhase));
        assert_eq!(ledger.merge_complete_set(1), Err(Error::InvalidPhase));
        assert_eq!(ledger.redeem(0, 8), Err(Error::InsufficientSupply));
        assert_eq!(ledger, resolved);
        Ok(())
    }

    #[test]
    fn split_overflow_refuses_atomically() {
        let mut ledger = CategoricalLedger::<2> {
            hoard_atoms: u64::MAX,
            supply: [u64::MAX; 2],
            phase: Phase::Open,
        };
        let before = ledger;
        assert_eq!(ledger.split_complete_set(1), Err(Error::ArithmeticOverflow));
        assert_eq!(ledger, before);
    }
}
